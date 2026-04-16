use std::collections::HashMap;
use std::future::{ready as future_ready, Future};
use std::pin::pin;
use std::str::FromStr;
use std::sync::Arc;
use std::task::{Context, Poll, Waker};

use lwk_common::Signer as _;
use lwk_signer::{SignError, SwSigner};
use lwk_simplicity::error::WalletAbiError;
use lwk_simplicity::wallet_abi::schema::{
    KeyStoreMeta, RequestPreview, RuntimeParams, TxCreateRequest, TxCreateResponse,
    TxEvaluateRequest, TxEvaluateResponse, WalletOutputAllocator, WalletOutputRequest,
    WalletOutputTemplate, WalletPrevoutResolver, WalletProviderMeta, WalletRequestSession,
    WalletRuntimeDeps, WalletSessionFactory,
};
use lwk_simplicity::wallet_abi::tx_resolution::runtime::Runtime as WalletAbiRuntime;
use lwk_test_util::{generate_mnemonic, generate_slip77, TestEnv, TestEnvBuilder};
use lwk_wollet::bitcoin::bip32::{DerivationPath, KeySource};
use lwk_wollet::bitcoin::PublicKey;
use lwk_wollet::blocking::BlockchainBackend;
use lwk_wollet::elements::bitcoin::secp256k1::Keypair;
use lwk_wollet::elements::pset::PartiallySignedTransaction;
use lwk_wollet::elements::{Address, OutPoint, Script, Transaction, TxOut, TxOutSecrets, Txid};
use lwk_wollet::secp256k1::schnorr::Signature;
use lwk_wollet::secp256k1::{Message, XOnlyPublicKey};
use lwk_wollet::{
    Chain, ElectrumClient, ElectrumUrl, ElementsNetwork, ExternalUtxo, WalletTx, Wollet,
    WolletBuilder, WolletDescriptor, EC,
};

const WALLET_ACCOUNT_PATH: &str = "m/84h/1h/0h";

type Bip32Derivations = HashMap<OutPoint, (PublicKey, KeySource)>;
type WalletRequestSessionParts = (WalletRequestSession, Bip32Derivations);

#[derive(Debug, thiserror::Error)]
enum LiveHarnessError {
    #[error(transparent)]
    Sign(#[from] SignError),
    #[error(transparent)]
    Wallet(#[from] lwk_wollet::Error),
    #[error("unsupported live harness operation: {0}")]
    Unsupported(&'static str),
}

impl From<LiveHarnessError> for WalletAbiError {
    fn from(error: LiveHarnessError) -> Self {
        WalletAbiError::InvalidRequest(error.to_string())
    }
}

struct LiveSigner {
    signer: SwSigner,
    xonly_public_key: XOnlyPublicKey,
}

impl LiveSigner {
    fn new(signer: SwSigner, xonly_public_key: XOnlyPublicKey) -> Self {
        Self {
            signer,
            xonly_public_key,
        }
    }
}

impl KeyStoreMeta for LiveSigner {
    type Error = LiveHarnessError;

    fn get_raw_signing_x_only_pubkey(&self) -> Result<XOnlyPublicKey, Self::Error> {
        Ok(self.xonly_public_key)
    }

    fn sign_pst(&self, pst: &mut PartiallySignedTransaction) -> Result<(), Self::Error> {
        self.signer.sign(pst)?;
        Ok(())
    }

    fn sign_schnorr(
        &self,
        message: Message,
        _xonly_public_key: XOnlyPublicKey,
    ) -> Result<Signature, Self::Error> {
        let root_xprv = self.signer.derive_xprv(&"m".parse().expect("root path"))?;
        let keypair = Keypair::from_secret_key(&EC, &root_xprv.private_key);

        Ok(EC.sign_schnorr(&message, &keypair))
    }
}

#[derive(Clone)]
struct LiveSessionFactory {
    session: WalletRequestSession,
}

impl LiveSessionFactory {
    fn new(session: WalletRequestSession) -> Self {
        Self { session }
    }
}

impl WalletSessionFactory for LiveSessionFactory {
    type Error = LiveHarnessError;

    fn open_wallet_request_session(
        &self,
    ) -> impl Future<Output = Result<WalletRequestSession, Self::Error>> + Send + '_ {
        future_ready(Ok(self.session.clone()))
    }
}

struct LiveWalletProvider {
    descriptor: WolletDescriptor,
    network: ElementsNetwork,
    electrum_url: String,
    bip32_derivations: Bip32Derivations,
}

impl LiveWalletProvider {
    fn new(
        descriptor: WolletDescriptor,
        network: ElementsNetwork,
        electrum_url: String,
        bip32_derivations: Bip32Derivations,
    ) -> Self {
        Self {
            descriptor,
            network,
            electrum_url,
            bip32_derivations,
        }
    }

    fn wallet_output_template(
        &self,
        script_pubkey: Script,
        address: Address,
    ) -> WalletOutputTemplate {
        WalletOutputTemplate {
            script_pubkey,
            blinding_pubkey: address.blinding_pubkey,
        }
    }
}

impl WalletPrevoutResolver for LiveWalletProvider {
    type Error = LiveHarnessError;

    fn get_bip32_derivation_pair(
        &self,
        out_point: &OutPoint,
    ) -> Result<Option<(PublicKey, KeySource)>, Self::Error> {
        Ok(self.bip32_derivations.get(out_point).cloned())
    }

    fn unblind(&self, _tx_out: &TxOut) -> Result<TxOutSecrets, Self::Error> {
        Err(LiveHarnessError::Unsupported(
            "wallet-provider unblind is not used by these integration tests",
        ))
    }

    fn get_tx_out(
        &self,
        outpoint: OutPoint,
    ) -> impl Future<Output = Result<TxOut, Self::Error>> + Send + '_ {
        let result = ElectrumUrl::from_str(&self.electrum_url)
            .map_err(|_| LiveHarnessError::Unsupported("invalid electrum url"))
            .and_then(|electrum_url| {
                let client = ElectrumClient::new(&electrum_url)?;
                let transaction = client.get_transaction(outpoint.txid)?;
                transaction
                    .output
                    .get(outpoint.vout as usize)
                    .cloned()
                    .ok_or(LiveHarnessError::Unsupported(
                        "provided prevout output index does not exist",
                    ))
            });
        future_ready(result)
    }
}

impl WalletOutputAllocator for LiveWalletProvider {
    type Error = LiveHarnessError;

    fn get_wallet_output_template(
        &self,
        _session: &WalletRequestSession,
        request: &WalletOutputRequest,
    ) -> Result<WalletOutputTemplate, Self::Error> {
        let params = self.network.address_params();
        let address = match request {
            WalletOutputRequest::Receive { index } => self.descriptor.address(*index, params)?,
            WalletOutputRequest::Change { index, .. } => self.descriptor.change(*index, params)?,
        };

        Ok(self.wallet_output_template(address.script_pubkey(), address))
    }
}

impl lwk_simplicity::wallet_abi::schema::WalletBroadcaster for LiveWalletProvider {
    type Error = LiveHarnessError;

    fn broadcast_transaction(
        &self,
        tx: &Transaction,
    ) -> impl Future<Output = Result<Txid, Self::Error>> + Send + '_ {
        let result = ElectrumUrl::from_str(&self.electrum_url)
            .map_err(|_| LiveHarnessError::Unsupported("invalid electrum url"))
            .and_then(|electrum_url| {
                let client = ElectrumClient::new(&electrum_url)?;
                client.broadcast(tx).map_err(Into::into)
            });
        future_ready(result)
    }
}

impl WalletProviderMeta for LiveWalletProvider {
    type Error = LiveHarnessError;

    fn get_bip32_derivation_pair(
        &self,
        out_point: &OutPoint,
    ) -> Result<Option<(PublicKey, KeySource)>, Self::Error> {
        WalletPrevoutResolver::get_bip32_derivation_pair(self, out_point)
    }

    fn unblind(&self, tx_out: &TxOut) -> Result<TxOutSecrets, Self::Error> {
        WalletPrevoutResolver::unblind(self, tx_out)
    }

    fn get_tx_out(
        &self,
        outpoint: OutPoint,
    ) -> impl Future<Output = Result<TxOut, Self::Error>> + Send + '_ {
        WalletPrevoutResolver::get_tx_out(self, outpoint)
    }

    fn get_wallet_output_template(
        &self,
        session: &WalletRequestSession,
        request: &WalletOutputRequest,
    ) -> Result<WalletOutputTemplate, Self::Error> {
        WalletOutputAllocator::get_wallet_output_template(self, session, request)
    }

    fn broadcast_transaction(
        &self,
        tx: &Transaction,
    ) -> impl Future<Output = Result<Txid, Self::Error>> + Send + '_ {
        lwk_simplicity::wallet_abi::schema::WalletBroadcaster::broadcast_transaction(self, tx)
    }
}

pub struct WalletAbiLiveHarness {
    pub env: TestEnv,
    pub network: ElementsNetwork,
    pub sender_wallet: Wollet,
    pub sender_xonly_public_key: XOnlyPublicKey,
    sender_signer: SwSigner,
    sender_descriptor: WolletDescriptor,
}

impl Default for WalletAbiLiveHarness {
    fn default() -> Self {
        Self::new()
    }
}

impl WalletAbiLiveHarness {
    pub fn new() -> Self {
        let env = TestEnvBuilder::from_env().with_electrum().build();
        let network = ElementsNetwork::default_regtest();
        let sender_signer = generate_signer();
        let sender_xonly_public_key = sender_signer.xpub().public_key.x_only_public_key().0;
        let sender_descriptor = wallet_descriptor(&sender_signer, &generate_slip77());
        let sender_wallet = WolletBuilder::new(network, sender_descriptor.clone())
            .build()
            .expect("sender wallet");

        Self {
            env,
            network,
            sender_signer,
            sender_descriptor,
            sender_wallet,
            sender_xonly_public_key,
        }
    }

    pub fn fund_sender_lbtc(&mut self, satoshi: u64) -> Txid {
        let address = self
            .sender_wallet
            .address(None)
            .expect("sender address")
            .address()
            .clone();
        let txid = self.env.elementsd_sendtoaddress(&address, satoshi, None);
        let mut client = electrum_client(&self.env);
        wait_for_tx(&mut self.sender_wallet, &mut client, &txid);
        txid
    }

    pub fn mine_and_sync_sender(&mut self, blocks: u32) {
        self.env.elementsd_generate(blocks);
        self.sync_sender();
    }

    pub fn sender_transaction(&self, txid: &Txid) -> Option<WalletTx> {
        self.sender_wallet
            .transaction(txid)
            .expect("sender tx lookup")
    }

    pub fn evaluate_request(
        &mut self,
        request: TxEvaluateRequest,
    ) -> Result<TxEvaluateResponse, WalletAbiError> {
        let signer = LiveSigner::new(self.sender_signer.clone(), self.sender_xonly_public_key);
        let wallet_deps = self.wallet_runtime_deps()?;

        ready(
            WalletAbiRuntime::<TxEvaluateRequest, _, _, _>::new(request, &signer, &wallet_deps)
                .evaluate_request(),
        )
    }

    pub fn process_request(
        &mut self,
        request: TxCreateRequest,
    ) -> Result<TxCreateResponse, WalletAbiError> {
        let signer = LiveSigner::new(self.sender_signer.clone(), self.sender_xonly_public_key);
        let wallet_deps = self.wallet_runtime_deps()?;

        ready(
            WalletAbiRuntime::<TxCreateRequest, _, _, _>::new(request, &signer, &wallet_deps)
                .process_request(),
        )
    }

    pub fn evaluate_then_process(
        &mut self,
        request_id: &str,
        params: RuntimeParams,
    ) -> Result<(RequestPreview, TxCreateResponse), WalletAbiError> {
        let preview = self
            .evaluate_request(
                TxEvaluateRequest::from_parts(request_id, self.network, params.clone())
                    .expect("evaluate request"),
            )?
            .preview
            .expect("preview");
        let response = self.process_request(
            TxCreateRequest::from_parts(request_id, self.network, params, true)
                .expect("process request"),
        )?;

        assert_eq!(
            response
                .preview()
                .expect("process preview accessor")
                .expect("process preview"),
            preview
        );

        Ok((preview, response))
    }

    fn sync_sender(&mut self) {
        let mut client = electrum_client(&self.env);
        sync(&mut self.sender_wallet, &mut client);
    }

    fn wallet_runtime_deps(
        &mut self,
    ) -> Result<WalletRuntimeDeps<LiveSessionFactory, LiveWalletProvider>, WalletAbiError> {
        self.sync_sender();
        let (session, bip32_derivations) =
            build_wallet_request_session(&self.sender_wallet, self.network, &self.sender_signer)?;

        Ok(WalletRuntimeDeps::new(
            LiveSessionFactory::new(session),
            LiveWalletProvider::new(
                self.sender_descriptor.clone(),
                self.network,
                self.env.electrum_url(),
                bip32_derivations,
            ),
        ))
    }
}

fn generate_signer() -> SwSigner {
    let mnemonic = generate_mnemonic();
    SwSigner::new(&mnemonic, false).unwrap()
}

fn wallet_descriptor(signer: &SwSigner, slip77_key: &str) -> WolletDescriptor {
    let account_path = DerivationPath::from_str(WALLET_ACCOUNT_PATH).expect("account path");
    let account_xpub = signer.derive_xpub(&account_path).expect("account xpub");
    let fingerprint = signer.fingerprint();
    let descriptor =
        format!("ct(slip77({slip77_key}),elwpkh([{fingerprint}/84h/1h/0h]{account_xpub}/<0;1>/*))");

    WolletDescriptor::from_str(&descriptor).expect("wallet descriptor")
}

fn electrum_client(env: &TestEnv) -> ElectrumClient {
    let electrum_url = ElectrumUrl::from_str(&env.electrum_url()).unwrap();
    ElectrumClient::new(&electrum_url).unwrap()
}

fn sync<S: BlockchainBackend>(wollet: &mut Wollet, client: &mut S) {
    let update = client.full_scan(wollet).unwrap();
    if let Some(update) = update {
        wollet.apply_update(update).unwrap();
    }
}

fn wait_for_tx<S: BlockchainBackend>(wollet: &mut Wollet, client: &mut S, txid: &Txid) {
    for _ in 0..120 {
        sync(wollet, client);
        let list = wollet.transactions().unwrap();
        if list.iter().any(|entry| &entry.txid == txid) {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(500));
    }
    panic!("Wallet does not have {txid} in its list");
}

fn build_wallet_request_session(
    wallet: &Wollet,
    network: ElementsNetwork,
    signer: &SwSigner,
) -> Result<WalletRequestSessionParts, LiveHarnessError> {
    let mut spendable_utxos = Vec::new();
    let mut bip32_derivations = HashMap::new();
    let fingerprint = signer.fingerprint();

    for utxo in wallet.utxos()? {
        let transaction = wallet
            .transaction(&utxo.outpoint.txid)?
            .ok_or(LiveHarnessError::Unsupported("wallet transaction missing"))?;
        let txout = transaction
            .tx
            .output
            .get(usize::try_from(utxo.outpoint.vout).expect("vout fits usize"))
            .cloned()
            .ok_or(LiveHarnessError::Unsupported("wallet tx output missing"))?;
        let derivation_path =
            wallet_derivation_path(chain_index(utxo.ext_int), utxo.wildcard_index).expect("path");
        let derived_xpub = signer.derive_xpub(&derivation_path)?;

        bip32_derivations.insert(
            utxo.outpoint,
            (
                PublicKey::new(derived_xpub.public_key),
                (fingerprint, derivation_path.clone()),
            ),
        );
        spendable_utxos.push(ExternalUtxo {
            outpoint: utxo.outpoint,
            txout,
            tx: None,
            unblinded: utxo.unblinded,
            max_weight_to_satisfy: wallet.max_weight_to_satisfy(),
        });
    }

    Ok((
        WalletRequestSession {
            session_id: "live-wallet-abi-session".to_string(),
            network,
            spendable_utxos: Arc::from(spendable_utxos),
        },
        bip32_derivations,
    ))
}

fn chain_index(chain: Chain) -> u32 {
    match chain {
        Chain::External => 0,
        Chain::Internal => 1,
    }
}

fn wallet_derivation_path(chain: u32, index: u32) -> Result<DerivationPath, lwk_wollet::Error> {
    DerivationPath::from_str(&format!("{WALLET_ACCOUNT_PATH}/{chain}/{index}")).map_err(Into::into)
}

fn ready<T>(future: impl Future<Output = T>) -> T {
    let waker = Waker::noop();
    let mut cx = Context::from_waker(waker);
    let mut future = pin!(future);
    match future.as_mut().poll(&mut cx) {
        Poll::Ready(value) => value,
        Poll::Pending => panic!("test future unexpectedly pending"),
    }
}
