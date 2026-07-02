use lwk_common::{Network, Signer};
use lwk_signer::SignError;
use lwk_wollet::{
    blocking::{BlockchainBackend, EsploraClient},
    elements::{
        pset::PartiallySignedTransaction, Address, AssetId, OutPoint, Script, Transaction, Txid,
    },
    ElectrumClient, Wollet, WolletBuilder, WolletDescriptor, EC,
};

use rand::thread_rng;

use simplex::transaction::{
    partial_input::IssuanceInput, FinalTransaction, PartialInput, PartialOutput, RequiredSignature,
    UTXO,
};

use lending_contracts::programs::issuance_factory::{IssuanceFactory, IssuanceFactoryParameters};
use lending_contracts::programs::program::SimplexProgram;
use lending_contracts::utils::get_random_seed;

use crate::lending::error::LendingError;
use crate::lending::network::to_simplicity_network;

enum AnyClient {
    Electrum(Box<ElectrumClient>),
    Esplora(EsploraClient),
}

impl AnyClient {
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
}

pub struct LendingSession {
    network: Network,
    indexer_url: String,
    wollet: Wollet,
    signer: Box<dyn Signer<Error = SignError>>,
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
            .ok_or_else(|| LendingError::Config("no suitable funding UTXO".into()))?;
        let input_value = funding_utxo.unblinded.value;
        let funding_outpoint = funding_utxo.outpoint;

        let tx_details = self
            .wollet
            .transaction(&funding_outpoint.txid)
            .map_err(LendingError::Wallet)?
            .ok_or_else(|| LendingError::Config("transaction for funding UTXO not found".into()))?;
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
            .ok_or_else(|| LendingError::Config("address has no blinding key".into()))?;

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
                .map_err(|e| LendingError::Config(format!("invalid blinding key: {e}")))?;
        ft.add_output(
            PartialOutput::new(change_script, change_value, policy_asset)
                .with_blinding_key(user_blinding_pk_btc),
        );

        let (mut pset, inp_txout_sec) = ft.extract_pst();
        let mut rng = thread_rng();

        pset.blind_last(&mut rng, &EC, &inp_txout_sec)
            .map_err(|e| LendingError::Config(format!("blinding error: {e}")))?;

        self.wollet
            .add_details(&mut pset)
            .map_err(LendingError::Wallet)?;

        self.signer
            .sign(&mut pset)
            .map_err(|e| LendingError::Config(format!("signing error: {e}")))?;

        let tx = self
            .wollet
            .finalize(&mut pset)
            .map_err(LendingError::Wallet)?;

        let txid = self.client.broadcast(&tx).map_err(LendingError::Wallet)?;

        let factory_address = lwk_wollet::elements::Address::from_script(
            &issuance_factory.get_script_pubkey(),
            None,
            self.network.address_params(),
        )
        .ok_or_else(|| LendingError::Config("invalid factory script_pubkey".into()))?;

        Ok(BorrowerAccountCreationResult {
            txid,
            funding_outpoint,
            factory_address,
            factory_auth_outpoint: OutPoint { txid, vout: 0 },
            issuance_factory_outpoint: OutPoint { txid, vout: 1 },
            issued_asset_id: issuance_details.asset_id,
        })
    }

    /// Create borrow offer
    ///
    /// # Errors
    /// Borrower account was not previously created
    pub fn borrower_create_offer(
        &self,
        _details: OfferDetails,
    ) -> Result<CreateBorrowTransaction, LendingError> {
        todo!()
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
}

/// Builder for creating a [`LendingSession`].
pub struct LendingSessionBuilder {
    network: Network,
    indexer_url: Option<String>,
    descriptor: WolletDescriptor,
    signer: Option<Box<dyn Signer<Error = SignError>>>,
    client: Option<AnyClient>,
}

impl LendingSessionBuilder {
    /// Create a new [`LendingSessionBuilder`] with required parameters.
    pub fn new(network: Network, descriptor: WolletDescriptor) -> Self {
        Self {
            network,
            descriptor,
            indexer_url: None,
            signer: None,
            client: None,
        }
    }

    pub fn set_indexer_url(mut self, indexer_url: String) -> Self {
        self.indexer_url = Some(indexer_url);
        self
    }

    pub fn set_signer(mut self, signer: Box<dyn Signer<Error = SignError>>) -> Self {
        self.signer = Some(signer);
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
        let signer = self
            .signer
            .ok_or_else(|| LendingError::Config("signer is required".into()))?;
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
            signer,
            client,
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
}

pub struct RepaymentDetails {
    pub amount_to_repay: u64,
}

pub struct BorrowerAccountParams {}

pub struct BorrowerAccountCreationResult {
    pub txid: Txid,
    pub funding_outpoint: OutPoint,
    pub factory_address: Address,
    pub factory_auth_outpoint: OutPoint,
    pub issuance_factory_outpoint: OutPoint,
    pub issued_asset_id: AssetId,
}

pub struct CreateBorrowTransaction {
    inner: PartiallySignedTransaction,
}

impl CreateBorrowTransaction {
    pub fn inner(&self) -> &PartiallySignedTransaction {
        &self.inner
    }
}
