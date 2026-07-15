use std::collections::HashMap;

use lwk_common::{calculate_fee, Network};
use lwk_wollet::{
    bitcoin,
    blocking::{BlockchainBackend, EsploraClient},
    elements::{
        confidential::{AssetBlindingFactor, ValueBlindingFactor},
        pset::{PartiallySignedTransaction, PsetBlindError},
        Address, AssetId, OutPoint, Script, Transaction, TxOutSecrets, Txid,
    },
    ElectrumClient, Wollet, WolletBuilder, WolletDescriptor, EC,
};

use rand::thread_rng;

use simplex::transaction::{
    partial_input::IssuanceInput, FinalTransaction, PartialInput, PartialOutput, RequiredSignature,
    UTXO,
};

use lending_contracts::programs::{
    issuance_factory::{IssuanceFactory, IssuanceFactoryParameters},
    lending::{LendingOfferParameters, OfferParameters},
};
use lending_contracts::programs::{lending::LendingOffer, program::SimplexProgram};
use lending_contracts::utils::get_random_seed;
use simplicityhl::WitnessValues;

use crate::lending::error::LendingError;
use crate::lending::network::to_simplicity_network;

use super::indexer::response::FactoryDetailsResponse;

enum AnyClient {
    Electrum(Box<ElectrumClient>),
    Esplora(EsploraClient),
}

impl AnyClient {
    #[allow(dead_code)]
    fn broadcast(&self, tx: &Transaction) -> Result<Txid, lwk_wollet::Error> {
        match self {
            AnyClient::Electrum(c) => c.broadcast(tx),
            AnyClient::Esplora(c) => c.broadcast(tx),
        }
    }

    fn full_scan(
        &mut self,
        wollet: &Wollet,
    ) -> Result<Option<lwk_wollet::Update>, lwk_wollet::Error> {
        match self {
            AnyClient::Electrum(c) => c.full_scan(wollet),
            AnyClient::Esplora(c) => c.full_scan(wollet),
        }
    }

    fn get_transaction(&self, txid: Txid) -> Result<Transaction, lwk_wollet::Error> {
        match self {
            AnyClient::Electrum(c) => c.get_transaction(txid),
            AnyClient::Esplora(c) => c.get_transaction(txid),
        }
    }
}

pub struct LendingSession {
    network: Network,
    indexer_url: String,
    wollet: Wollet,
    client: AnyClient,
}

impl LendingSession {
    pub fn builder(network: Network, descriptor: WolletDescriptor) -> LendingSessionBuilder {
        LendingSessionBuilder::new(network, descriptor)
    }

    pub fn network(&self) -> Network {
        self.network
    }

    pub fn indexer_url(&self) -> &str {
        &self.indexer_url
    }

    /// One-time action from every user to prepare for creating an offer.
    pub fn borrower_prepare(
        &self,
        _params: BorrowerAccountParams,
        fee_rate: f32,
    ) -> Result<BorrowerAccountCreationResult, LendingError> {
        const ISSUANCE_AMOUNT: u64 = 2;
        const FACTORY_AUTH_AMOUNT: u64 = 1;
        const ISSUANCE_FACTORY_AMOUNT: u64 = 1;
        const FEE_ESTIMATE: u64 = 250;

        let policy_asset = *self.network.policy_asset();

        let funding_utxo = self.get_utxo(policy_asset, FEE_ESTIMATE, &[])?;

        let parameters = IssuanceFactoryParameters {
            issuing_utxos_count: 2,
            reissuance_flags: 0,
            network: to_simplicity_network(self.network),
        };
        let issuance_factory = IssuanceFactory::new(parameters);

        let mut ft = FinalTransaction::new();
        let entropy = get_random_seed();

        let issuance_details = ft.add_issuance_input(
            PartialInput::new(funding_utxo.clone()),
            IssuanceInput::new_issuance(ISSUANCE_AMOUNT, 0, entropy),
            RequiredSignature::NativeEcdsa,
        );

        let change_addr = self.wollet.change(None)?;
        let change_script = change_addr.address().script_pubkey();
        let change_blinding =
            change_addr
                .address()
                .blinding_pubkey
                .ok_or(LendingError::Generic(
                    "change address has no blinding key".into(),
                ))?;
        let change_pk = bitcoin::PublicKey::from(change_blinding);

        let user_addr = self.wollet.address(None)?;
        let user_script = user_addr.address().script_pubkey();

        ft.add_output(PartialOutput::new(
            user_script,
            FACTORY_AUTH_AMOUNT,
            issuance_details.asset_id,
        ));

        issuance_factory.attach_creation(
            &mut ft,
            issuance_details.asset_id,
            ISSUANCE_FACTORY_AMOUNT,
        );

        let _ = self.add_fee(&mut ft, change_script, change_pk, fee_rate)?;

        let (mut pset, inp_txout_sec) = ft.extract_pst();
        let mut rng = thread_rng();

        pset.blind_last(&mut rng, &EC, &inp_txout_sec)
            .map_err(|e| LendingError::Generic(format!("blinding error: {e}")))?;

        self.wollet
            .add_details(&mut pset)
            .map_err(LendingError::Wallet)?;

        let factory_address = lwk_wollet::elements::Address::from_script(
            &issuance_factory.get_script_pubkey(),
            None,
            self.network.address_params(),
        )
        .ok_or_else(|| LendingError::Generic("invalid factory script_pubkey".into()))?;

        Ok(BorrowerAccountCreationResult {
            pset,
            factory_address,
            issued_asset_id: issuance_details.asset_id,
        })
    }

    // TODO: we shouldn't have so many network calls in this function
    // TODO: we should attach fee rate to the function/struct
    /// Create a borrow offer
    ///
    /// # Errors
    /// Returns an error if the wallet has no suitable UTXOs or the lending transaction construction
    /// fails.
    pub fn borrower_create_offer(
        &mut self,
        details: OfferDetails,
        factory: FactoryDetails,
        fee_rate: f32,
    ) -> Result<CreateBorrowTransaction, LendingError> {
        const NFT_AMOUNT: u64 = 1;
        const FEE_ESTIMATE: u64 = 250;

        let policy_asset = *self.network.policy_asset();

        let issuance_factory_params = IssuanceFactoryParameters {
            issuing_utxos_count: 2,
            reissuance_flags: 0,
            network: to_simplicity_network(self.network),
        };
        let issuance_factory = IssuanceFactory::new(issuance_factory_params);

        // Fetch the prepare transaction to obtain the raw txouts for the factory utxos.
        let prepare_tx = self
            .client
            .get_transaction(factory.auth_utxo.txid)
            .map_err(|e| LendingError::Generic(format!("failed to fetch prepare tx: {e}")))?;

        let program_tx = self
            .client
            .get_transaction(factory.program_utxo.txid)
            .map_err(|e| LendingError::Generic(format!("failed to fetch program tx: {e}")))?;

        let auth_txout = prepare_tx
            .output
            .get(factory.auth_utxo.vout as usize)
            .ok_or_else(|| LendingError::Generic("auth vout out of bounds".into()))?
            .clone();

        let auth_script = auth_txout.script_pubkey.clone();
        let program_txout = program_tx
            .output
            .get(factory.program_utxo.vout as usize)
            .ok_or_else(|| LendingError::Generic("program vout out of bounds".into()))?
            .clone();

        let collateral_utxo =
            self.get_utxo(details.collateral_asset_id, details.collateral_amount, &[])?;

        let fee_funding_utxo =
            self.get_utxo(policy_asset, FEE_ESTIMATE, &[collateral_utxo.outpoint])?;

        let change_addr = self.wollet.change(None)?;
        let change_script = change_addr.address().script_pubkey();
        let change_blinding =
            change_addr
                .address()
                .blinding_pubkey
                .ok_or(LendingError::Generic(
                    "change address has no blinding key".into(),
                ))?;
        let change_pk = bitcoin::PublicKey::from(change_blinding);

        let user_addr = self.wollet.address(None)?;
        let user_script = user_addr.address().script_pubkey();

        // Use shared entropy for both NFTs
        let nfts_entropy = get_random_seed();

        // Build the transaction
        let mut ft = FinalTransaction::new();

        // Input 0: auth UTXO
        ft.add_input(
            PartialInput::new(UTXO {
                outpoint: factory.auth_utxo,
                txout: auth_txout,
                secrets: Some(TxOutSecrets {
                    asset: factory.factory_asset_id,
                    asset_bf: AssetBlindingFactor::zero(),
                    value: 1,
                    value_bf: ValueBlindingFactor::zero(),
                }),
            }),
            RequiredSignature::NativeEcdsa,
        );

        // Output 0: auth UTXO
        ft.add_output(PartialOutput::new(
            auth_script.clone(),
            NFT_AMOUNT,
            factory.factory_asset_id,
        ));

        // - Input 1: program UTXO with borrower NFT issuance
        // - Output 1: program UTXO
        let program_issuance = IssuanceInput::new_issuance(NFT_AMOUNT, 0, nfts_entropy);
        let borrower_nft_details = issuance_factory.attach_assets_issuance(
            &mut ft,
            UTXO {
                outpoint: factory.program_utxo,
                txout: program_txout,
                secrets: Some(TxOutSecrets {
                    asset: factory.factory_asset_id,
                    asset_bf: AssetBlindingFactor::zero(),
                    value: 1,
                    value_bf: ValueBlindingFactor::zero(),
                }),
            },
            program_issuance,
        );

        // Output 2: borrower NFT to user (from the factory issuance)
        ft.add_output(PartialOutput::new(
            user_script.clone(),
            NFT_AMOUNT,
            borrower_nft_details.asset_id,
        ));

        // Input 2: collateral UTXO with lender NFT issuance
        let lender_nft_issuance = IssuanceInput::new_issuance(NFT_AMOUNT, 0, nfts_entropy);
        let lender_nft_details = ft.add_issuance_input(
            PartialInput::new(collateral_utxo.clone()),
            lender_nft_issuance,
            RequiredSignature::NativeEcdsa,
        );

        // Input 3: fee funding UTXO
        ft.add_input(
            PartialInput::new(fee_funding_utxo.clone()),
            RequiredSignature::NativeEcdsa,
        );

        // Build the LendingOffer
        let lending_offer_params = LendingOfferParameters {
            collateral_asset_id: details.collateral_asset_id,
            principal_asset_id: details.principal_asset_id,
            borrower_nft_asset_id: borrower_nft_details.asset_id,
            lender_nft_asset_id: lender_nft_details.asset_id,
            protocol_fee_keeper_asset_id: details.protocol_fee_keeper_asset_id,
            offer_parameters: OfferParameters {
                collateral_amount: details.collateral_amount,
                principal_amount: details.principal_amount,
                loan_expiration_time: details.loan_expiration_time,
                principal_interest_rate: details.principal_interest_rate,
            },
            network: to_simplicity_network(self.network),
        };
        let lending_offer = LendingOffer::new_pending(lending_offer_params);

        // - Output 3: lender NFT with ScriptAuth
        // - Output 4: OP_RETURN metadata
        // - Output 5: lending covenant collateral
        lending_offer.attach_creation(&mut ft);

        // Add collateral change output
        if collateral_utxo.amount() > details.collateral_amount {
            ft.add_output(
                PartialOutput::new(
                    change_script.clone(),
                    collateral_utxo.amount() - details.collateral_amount,
                    details.collateral_asset_id,
                )
                .with_blinding_key(change_pk),
            );
        }

        // Add fee
        let _ = self.add_fee(&mut ft, change_script, change_pk, fee_rate)?;

        // Extract
        let (mut pset, inp_txout_sec) = ft.extract_pst();

        // Blind, add details
        let mut rng = thread_rng();
        pset.blind_last(&mut rng, &EC, &inp_txout_sec)
            .map_err(|e| LendingError::Generic(format!("blinding error: {e}")))?;

        self.wollet
            .add_details(&mut pset)
            .map_err(LendingError::Wallet)?;

        // Finalize Simplicity program inputs on the PSET
        self.finalize_program_inputs(&ft, &mut pset)?;

        Ok(CreateBorrowTransaction { pset })
    }

    pub fn fully_repay_loan(&self, _details: RepaymentDetails) -> Result<(), LendingError> {
        todo!()
    }

    pub fn partially_repay_loan(&self, _details: RepaymentDetails) -> Result<(), LendingError> {
        todo!()
    }

    pub fn cancel_offer(&self) -> Result<(), LendingError> {
        todo!()
    }

    /// Accept a pending borrow offer as a lender.
    ///
    /// # Errors
    /// Returns an error if the wallet has no suitable principal or fee UTXOs, the pending offer
    /// creation transaction cannot be fetched, or the lending transaction construction fails.
    pub fn accept_offer(
        &mut self,
        details: AcceptOfferDetails,
        fee_rate: f32,
    ) -> Result<AcceptOfferTransaction, LendingError> {
        const FEE_ESTIMATE: u64 = 250;
        const LENDER_NFT_VOUT: usize = 3;
        const COVENANT_VOUT: usize = 5;

        let policy_asset = *self.network.policy_asset();

        // Fetch the pending offer creation transaction
        let creation_tx = self
            .client
            .get_transaction(details.pending_offer_creation_txid)?;

        // Reconstruct the LendingOffer from the creation transaction
        let mut offer = LendingOffer::try_from_tx(
            &creation_tx,
            details.protocol_fee_keeper_asset_id,
            to_simplicity_network(self.network),
        )
        .map_err(|e| {
            LendingError::Generic(format!(
                "failed to parse pending offer from creation tx: {e}"
            ))
        })?;

        let offer_params = *offer.get_parameters();

        // Get covenant UTXO
        let covenant_txout = creation_tx
            .output
            .get(COVENANT_VOUT)
            .ok_or_else(|| {
                LendingError::Generic("covenant output not found in creation tx".into())
            })?
            .clone();

        let pending_offer_utxo = UTXO {
            outpoint: OutPoint {
                txid: details.pending_offer_creation_txid,
                vout: COVENANT_VOUT as u32,
            },
            txout: covenant_txout,
            secrets: None,
        };

        // Get lender NFT UTXO
        let lender_nft_txout = creation_tx
            .output
            .get(LENDER_NFT_VOUT)
            .ok_or_else(|| {
                LendingError::Generic("lender NFT output not found in creation tx".into())
            })?
            .clone();

        let lender_nft_utxo = UTXO {
            outpoint: OutPoint {
                txid: details.pending_offer_creation_txid,
                vout: LENDER_NFT_VOUT as u32,
            },
            txout: lender_nft_txout,
            secrets: None,
        };

        // Find collateral UTXO via wollet
        let principal_utxo = self.get_utxo(
            offer_params.principal_asset_id,
            offer_params.offer_parameters.principal_amount,
            &[],
        )?;

        // Select a UTXO for a fee
        // TODO: don't select if collateral_asset_id == policy_asset
        let fee_funding_utxo =
            self.get_utxo(policy_asset, FEE_ESTIMATE, &[principal_utxo.outpoint])?;

        // Derive change address
        let change_addr = self.wollet.change(None)?;
        let change_script = change_addr.address().script_pubkey();
        let change_blinding =
            change_addr
                .address()
                .blinding_pubkey
                .ok_or(LendingError::Generic(
                    "change address has no blinding key".into(),
                ))?;

        let change_pk = bitcoin::PublicKey::from(change_blinding);

        // Derive user address for NFT
        // We could use change address but it's technically is not a change.
        let user_addr = self.wollet.address(None)?;
        let user_script = user_addr.address().script_pubkey();

        // Build transaction
        let mut ft = FinalTransaction::new();

        // - Input 0: pending offer covenant program input
        // - Input 1: lender NFT ScriptAuth unlock
        // - Output 0: active offer covenant output (collateral)
        // - Output 1: principal asset auth output
        offer.attach_acceptance(&mut ft, pending_offer_utxo, lender_nft_utxo);

        // Input 2: principal UTXO
        ft.add_input(
            PartialInput::new(principal_utxo.clone()),
            RequiredSignature::NativeEcdsa,
        );

        // Input 3: fee funding UTXO
        ft.add_input(
            PartialInput::new(fee_funding_utxo.clone()),
            RequiredSignature::NativeEcdsa,
        );

        // Output 2: Return lender NFT to lender
        ft.add_output(PartialOutput::new(
            user_script.clone(),
            1,
            offer_params.lender_nft_asset_id,
        ));

        // Optionaly change output for principal_asset_id
        if principal_utxo.amount() > offer_params.offer_parameters.principal_amount {
            ft.add_output(
                PartialOutput::new(
                    change_script.clone(),
                    principal_utxo.amount() - offer_params.offer_parameters.principal_amount,
                    offer_params.principal_asset_id,
                )
                .with_blinding_key(change_pk),
            );
        }

        // Add fee
        // Optionaly change output for policy asset
        let _ = self.add_fee(&mut ft, change_script, change_pk, fee_rate)?;

        // Extract PSET, blind, add wallet details
        let (mut pset, inp_txout_sec) = ft.extract_pst();

        let mut rng = thread_rng();
        pset.blind_last(&mut rng, &EC, &inp_txout_sec)
            .map_err(|e| LendingError::Generic(format!("blinding error: {e}")))?;

        self.wollet
            .add_details(&mut pset)
            .map_err(LendingError::Wallet)?;

        // Finalize Simplicity program inputs on the PSET
        self.finalize_program_inputs(&ft, &mut pset)?;

        Ok(AcceptOfferTransaction { pset })
    }

    pub fn claim_partial_repayment(&self) -> Result<(), LendingError> {
        todo!()
    }

    pub fn liquidate_offer(&self) -> Result<(), LendingError> {
        todo!()
    }

    pub fn sync(&mut self) -> Result<(), LendingError> {
        let update = self.client.full_scan(&self.wollet)?;
        if let Some(update) = update {
            self.wollet.apply_update(update)?;
        }
        Ok(())
    }

    /// Finalizes PSET with wollet
    ///
    /// In the future, this method would also append required witness for simplicity outputs.
    pub fn finalize(
        &self,
        pset: &mut PartiallySignedTransaction,
    ) -> Result<Transaction, LendingError> {
        self.wollet.finalize(pset).map_err(LendingError::Wallet)
    }

    /// Finalize Simplicity program inputs on the PSET.
    ///
    /// This function is applicable only for simplicity-lending, because it doesn't use any
    /// simplicity programs with signatures.
    fn finalize_program_inputs(
        &self,
        ft: &FinalTransaction,
        pset: &mut PartiallySignedTransaction,
    ) -> Result<(), LendingError> {
        let simplex_network = to_simplicity_network(self.network);

        for (index, final_input) in ft.inputs().iter().enumerate() {
            let Some(program_input) = &final_input.program_input else {
                continue;
            };

            let witness_map: HashMap<simplicityhl::str::WitnessName, simplicityhl::Value> =
                program_input
                    .witness
                    .build_witness()
                    .iter()
                    .map(|(k, v)| (k.clone(), v.clone()))
                    .collect();

            let witness_values = WitnessValues::from(witness_map);

            let pruned_witness = program_input
                .program
                .finalize(pset, &witness_values, index, &simplex_network)
                .map_err(|e| LendingError::Generic(format!("program finalization error: {e}")))?;

            pset.inputs_mut()[index].final_script_witness = Some(pruned_witness);
        }

        Ok(())
    }

    // TODO: we could add multiple utxo with sum > sats for more flexibility
    /// Return simplex [`UTXO`] with given asset ID and a amount higher than given sats.
    /// Searches for suitable UTXO inside wallet cache.
    ///
    /// # Errors
    ///
    /// Return an error if suitable UTXO not found.
    fn get_utxo(
        &self,
        asset_id: AssetId,
        sats: u64,
        excluded: &[OutPoint],
    ) -> Result<UTXO, LendingError> {
        let utxos = self.wollet.utxos().map_err(LendingError::Wallet)?;
        let utxo = utxos
            .into_iter()
            .filter(|u| {
                u.unblinded.asset == asset_id
                    && u.unblinded.value >= sats
                    && !excluded.contains(&u.outpoint)
            })
            .min_by_key(|u| u.unblinded.value)
            .ok_or(LendingError::Generic(format!(
                "No suitable UTXO found for {asset_id} with amount {sats}"
            )))?;
        let txid = &utxo.outpoint.txid;

        let tx = self
            .wollet
            .transaction(txid)?
            .ok_or(LendingError::Generic(format!(
                "transaction with txid {txid} was not found in wallet"
            )))?;
        let vout = utxo.outpoint.vout;

        let txout = tx
            .tx
            .output
            .get(vout as usize)
            .ok_or(LendingError::Generic(format!(
                "Output for txid {txid} with vout {vout} was not found"
            )))?;

        Ok(UTXO {
            outpoint: utxo.outpoint,
            txout: txout.clone(),
            secrets: Some(utxo.unblinded),
        })
    }

    // TODO: should we add more policy_asset inputs here if fee is not fully covered?
    /// Estimate the fee for the given [`FinalTransaction`] and adds fee and change output.
    /// `fee_rate` is fee rate in sats/kvb.
    ///
    /// Returns the computed fee in satoshis, or an error if funds are insufficient.
    fn add_fee(
        &self,
        ft: &mut FinalTransaction,
        change_script: Script,
        change_pk: bitcoin::PublicKey,
        fee_rate: f32,
    ) -> Result<u64, LendingError> {
        let simplex_network = to_simplicity_network(self.network);
        let policy_asset = *self.network.policy_asset();

        let (mut pset, inp_txout_sec) = ft.extract_pst();
        let mut rng = thread_rng();
        pset.blind_last(&mut rng, &EC, &inp_txout_sec)
            .or_else(|e| match e {
                // We ignoring this error because we can have simplicity-only outputs
                PsetBlindError::AtleastOneOutputBlind => Ok(()),
                other => Err(other),
            })
            .map_err(lwk_wollet::Error::from)?;
        let tx = pset.extract_tx().map_err(lwk_wollet::Error::from)?;

        let weight = tx.discount_weight();
        let fee = calculate_fee(weight, fee_rate);

        let available_delta =
            u64::try_from(ft.calculate_fee_delta(&simplex_network)).map_err(|_| {
                LendingError::Generic("fee delta is negative, no L-BTC available for fee".into())
            })?;

        if available_delta < fee {
            return Err(LendingError::Generic(format!(
                "insufficient L-BTC for fee: need {fee}, have {available_delta}"
            )));
        }

        let change = available_delta - fee;

        ft.add_output(PartialOutput::new(Script::default(), fee, policy_asset));
        if change != 0 {
            ft.add_output(
                PartialOutput::new(change_script, change, policy_asset)
                    .with_blinding_key(change_pk),
            );
        }

        Ok(fee)
    }
}

/// Builder for creating a [`LendingSession`].
pub struct LendingSessionBuilder {
    network: Network,
    indexer_url: Option<String>,
    descriptor: WolletDescriptor,
    client: Option<AnyClient>,
}

impl LendingSessionBuilder {
    /// Create a new [`LendingSessionBuilder`] with required parameters.
    pub fn new(network: Network, descriptor: WolletDescriptor) -> Self {
        Self {
            network,
            descriptor,
            indexer_url: None,
            client: None,
        }
    }

    pub fn set_indexer_url(mut self, indexer_url: String) -> Self {
        self.indexer_url = Some(indexer_url);
        self
    }

    pub fn set_electrum_client(mut self, client: ElectrumClient) -> Self {
        self.client = Some(AnyClient::Electrum(Box::new(client)));
        self
    }

    pub fn set_esplora_client(mut self, client: EsploraClient) -> Self {
        self.client = Some(AnyClient::Esplora(client));
        self
    }

    /// Build the [`LendingSession`].
    pub fn build(self) -> Result<LendingSession, LendingError> {
        let client = self
            .client
            .ok_or_else(|| LendingError::Config("blockchain client is required".into()))?;

        let indexer_url = self
            .indexer_url
            .ok_or_else(|| LendingError::Config("indexer url is required".into()))?;

        let wollet = WolletBuilder::new(self.network, self.descriptor)
            .build()
            .map_err(LendingError::Wallet)?;
        Ok(LendingSession {
            network: self.network,
            wollet,
            indexer_url,
            client,
        })
    }
}

pub struct FactoryDetails {
    factory_asset_id: AssetId,
    auth_utxo: OutPoint,
    program_utxo: OutPoint,
}

impl TryFrom<FactoryDetailsResponse> for FactoryDetails {
    type Error = LendingError;

    fn try_from(value: FactoryDetailsResponse) -> Result<Self, Self::Error> {
        if matches!(
            value.status,
            super::indexer::response::FactoryStatus::Removed
        ) {
            return Err(LendingError::CannotParseFactory(
                "factory status is Removed".to_string(),
            ));
        }

        let auth_utxo = value.auth_utxo.ok_or(LendingError::CannotParseFactory(
            "auth_utxo is missing".to_string(),
        ))?;
        let program_utxo = value.program_utxo.ok_or(LendingError::CannotParseFactory(
            "program_utxo is missing".to_string(),
        ))?;

        Ok(Self {
            factory_asset_id: value.factory_asset_id,
            auth_utxo: OutPoint {
                txid: auth_utxo.txid,
                vout: auth_utxo.vout,
            },
            program_utxo: OutPoint {
                txid: program_utxo.txid,
                vout: program_utxo.vout,
            },
        })
    }
}

pub struct OfferDetails {
    pub principal_asset_id: AssetId,
    pub principal_amount: u64,
    pub collateral_asset_id: AssetId,
    pub collateral_amount: u64,
    pub loan_expiration_time: u32,
    pub principal_interest_rate: u16,
    pub protocol_fee_keeper_asset_id: AssetId,
}

pub struct AcceptOfferDetails {
    pub pending_offer_creation_txid: Txid,
    pub protocol_fee_keeper_asset_id: AssetId,
}

pub struct RepaymentDetails {
    pub amount_to_repay: u64,
}

pub struct BorrowerAccountParams {}

pub struct BorrowerAccountCreationResult {
    pset: PartiallySignedTransaction,
    pub factory_address: Address,
    pub issued_asset_id: AssetId,
}

impl BorrowerAccountCreationResult {
    pub fn inner(&self) -> &PartiallySignedTransaction {
        &self.pset
    }
    pub fn into_inner(self) -> PartiallySignedTransaction {
        self.pset
    }
}

pub struct CreateBorrowTransaction {
    pset: PartiallySignedTransaction,
}

impl CreateBorrowTransaction {
    pub fn inner(&self) -> &PartiallySignedTransaction {
        &self.pset
    }

    pub fn into_inner(self) -> PartiallySignedTransaction {
        self.pset
    }
}

pub struct AcceptOfferTransaction {
    pset: PartiallySignedTransaction,
}

impl AcceptOfferTransaction {
    pub fn inner(&self) -> &PartiallySignedTransaction {
        &self.pset
    }

    pub fn into_inner(self) -> PartiallySignedTransaction {
        self.pset
    }
}
