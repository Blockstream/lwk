use std::collections::HashMap;

use lwk_common::{calculate_fee, Network};
use lwk_wollet::{
    bitcoin,
    blocking::{BlockchainBackend, EsploraClient},
    elements::{
        confidential::{AssetBlindingFactor, ValueBlindingFactor},
        pset::PartiallySignedTransaction,
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
    ) -> Result<BorrowerAccountCreationResult, LendingError> {
        // TODO: we should estimate fee dynamically
        const FEE: u64 = 250;
        const ISSUANCE_AMOUNT: u64 = 2;
        const FACTORY_AUTH_AMOUNT: u64 = 1;
        const ISSUANCE_FACTORY_AMOUNT: u64 = 1;

        let policy_asset = *self.network.policy_asset();

        let utxos = self.wollet.utxos().map_err(LendingError::Wallet)?;
        let funding_utxo = utxos
            .iter()
            .filter(|u| u.unblinded.asset == policy_asset && u.unblinded.value > FEE)
            .min_by_key(|u| u.unblinded.value)
            .ok_or_else(|| LendingError::Generic("no suitable funding UTXO".into()))?;
        let input_value = funding_utxo.unblinded.value;
        let funding_outpoint = funding_utxo.outpoint;

        let tx_details = self
            .wollet
            .transaction(&funding_outpoint.txid)
            .map_err(LendingError::Wallet)?
            .ok_or_else(|| {
                LendingError::Generic("transaction for funding UTXO not found".into())
            })?;
        let txout = tx_details.tx.output[funding_outpoint.vout as usize].clone();

        let parameters = IssuanceFactoryParameters {
            issuing_utxos_count: 2,
            reissuance_flags: 0,
            network: to_simplicity_network(self.network),
        };
        let issuance_factory = IssuanceFactory::new(parameters);

        let mut ft = FinalTransaction::new();
        let entropy = get_random_seed();

        let utxo = UTXO {
            outpoint: funding_outpoint,
            txout,
            secrets: Some(funding_utxo.unblinded),
        };
        let issuance_details = ft.add_issuance_input(
            PartialInput::new(utxo),
            IssuanceInput::new_issuance(ISSUANCE_AMOUNT, 0, entropy),
            RequiredSignature::NativeEcdsa,
        );

        let address_result = self.wollet.address(None).map_err(LendingError::Wallet)?;
        let user_script = address_result.address().script_pubkey();
        let user_blinding_pk = address_result
            .address()
            .blinding_pubkey
            .ok_or_else(|| LendingError::Generic("address has no blinding key".into()))?;

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

        let change_value = input_value - FEE;
        ft.add_output(PartialOutput::new(Script::default(), FEE, policy_asset));

        let change_script = self
            .wollet
            .address(None)
            .map_err(LendingError::Wallet)?
            .address()
            .script_pubkey();
        let user_blinding_pk_btc =
            lwk_wollet::elements::bitcoin::PublicKey::from_slice(&user_blinding_pk.serialize())
                .map_err(|e| LendingError::Generic(format!("invalid blinding key: {e}")))?;
        ft.add_output(
            PartialOutput::new(change_script, change_value, policy_asset)
                .with_blinding_key(user_blinding_pk_btc),
        );

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
    ) -> Result<CreateBorrowTransaction, LendingError> {
        // TODO: we should estimate fee dynamically
        const FEE: u64 = 250;
        const NFT_AMOUNT: u64 = 1;

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

        // TODO: should we select this UTXOs outside of the session?
        // Find collateral UTXO via wollet
        let utxos = self.wollet.utxos().map_err(LendingError::Wallet)?;
        let collateral_utxo = utxos
            .iter()
            .filter(|u| {
                u.unblinded.asset == details.collateral_asset_id
                    && u.unblinded.value >= details.collateral_amount
            })
            .min_by_key(|u| u.unblinded.value)
            .ok_or_else(|| LendingError::Generic("no suitable collateral UTXO in wallet".into()))?;
        let collateral_value = collateral_utxo.unblinded.value;

        let collateral_tx = self
            .client
            .get_transaction(collateral_utxo.outpoint.txid)
            .map_err(|e| LendingError::Generic(format!("failed to fetch collateral tx: {e}")))?;
        let collateral_txout = collateral_tx.output[collateral_utxo.outpoint.vout as usize].clone();

        // Select a UTXO for a fee
        let fee_funding_utxo = utxos
            .iter()
            .filter(|u| {
                u.unblinded.asset == policy_asset
                    && u.unblinded.value > FEE
                    && u.outpoint != collateral_utxo.outpoint
            })
            .min_by_key(|u| u.unblinded.value)
            .ok_or_else(|| {
                LendingError::Generic("no suitable fee funding UTXO in wallet".into())
            })?;
        let fee_funding_value = fee_funding_utxo.unblinded.value;

        let fee_funding_tx = self
            .client
            .get_transaction(fee_funding_utxo.outpoint.txid)
            .map_err(|e| LendingError::Generic(format!("failed to fetch fee funding tx: {e}")))?;
        let fee_funding_txout =
            fee_funding_tx.output[fee_funding_utxo.outpoint.vout as usize].clone();

        // Derive the user's next receive address for auth return and change
        let address_result = self.wollet.address(None).map_err(LendingError::Wallet)?;
        let user_script = address_result.address().script_pubkey();
        let user_blinding_pk = address_result
            .address()
            .blinding_pubkey
            .ok_or_else(|| LendingError::Generic("address has no blinding key".into()))?;

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
            PartialInput::new(UTXO {
                outpoint: collateral_utxo.outpoint,
                txout: collateral_txout,
                secrets: Some(collateral_utxo.unblinded),
            }),
            lender_nft_issuance,
            RequiredSignature::NativeEcdsa,
        );

        // Input 3: fee funding UTXO
        ft.add_input(
            PartialInput::new(UTXO {
                outpoint: fee_funding_utxo.outpoint,
                txout: fee_funding_txout,
                secrets: Some(fee_funding_utxo.unblinded),
            }),
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

        let user_blinding_pk_btc =
            lwk_wollet::elements::bitcoin::PublicKey::from_slice(&user_blinding_pk.serialize())
                .map_err(|e| LendingError::Generic(format!("invalid blinding key: {e}")))?;

        // Add fee output
        ft.add_output(PartialOutput::new(Script::default(), FEE, policy_asset));

        // Add fee change output
        let fee_change_value = fee_funding_value - FEE;
        ft.add_output(
            PartialOutput::new(user_script.clone(), fee_change_value, policy_asset)
                .with_blinding_key(user_blinding_pk_btc),
        );

        // Add collateral change output
        let collateral_change_value = collateral_value - details.collateral_amount;
        if collateral_change_value > 0 {
            ft.add_output(
                PartialOutput::new(
                    user_script.clone(),
                    collateral_change_value,
                    details.collateral_asset_id,
                )
                .with_blinding_key(user_blinding_pk_btc),
            );
        }

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

    pub fn accept_offer(&self) -> Result<(), LendingError> {
        todo!()
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

    /// Estimate the fee for the given [`FinalTransaction`] and adds fee and change output
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
