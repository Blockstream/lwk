use std::future::Future;
use std::pin::pin;
use std::str::FromStr;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::task::{Context, Poll, Waker};

use lwk_common::Signer as _;
use lwk_signer::SwSigner;
use lwk_simplicity::error::WalletAbiError;
use lwk_simplicity::wallet_abi::schema::OutputSchema;
use lwk_simplicity::wallet_abi::{
    AssetVariant, BlinderVariant, KeyStoreMeta, LockVariant, PreviewAssetDelta, RuntimeParams,
    TxCreateRequest, TxCreateResponse, TxEvaluateRequest, TxEvaluateResponse, WalletAbiProvider,
    WalletAbiProviderBuilder, WalletBroadcaster, WalletCapabilities, WalletOutputAllocator,
    WalletOutputRequest, WalletOutputTemplate, WalletPrevoutResolver, WalletReceiveAddressProvider,
    WalletRequestSession, WalletSessionFactory, GET_RAW_SIGNING_X_ONLY_PUBKEY_METHOD,
    GET_SIGNER_RECEIVE_ADDRESS_METHOD, WALLET_ABI_EVALUATE_REQUEST_METHOD,
    WALLET_ABI_GET_CAPABILITIES_METHOD, WALLET_ABI_PROCESS_REQUEST_METHOD,
};
use lwk_wollet::bitcoin::bip32::{DerivationPath, Fingerprint, KeySource};
use lwk_wollet::bitcoin::PublicKey;
use lwk_wollet::elements::confidential::{Asset as ConfidentialAsset, Nonce, Value};
use lwk_wollet::elements::pset::PartiallySignedTransaction;
use lwk_wollet::elements::secp256k1_zkp::PublicKey as BlindingPublicKey;
use lwk_wollet::elements::{
    Address, AssetId, OutPoint, Transaction, TxOut, TxOutSecrets, TxOutWitness, Txid,
};
use lwk_wollet::secp256k1::schnorr::Signature;
use lwk_wollet::secp256k1::{Message, XOnlyPublicKey};
use lwk_wollet::ExternalUtxo;
use serde::de::DeserializeOwned;

#[derive(Debug, thiserror::Error)]
enum ProviderError {
    #[error(transparent)]
    Sign(#[from] lwk_signer::SignError),
    #[error("test provider error")]
    Test,
}

impl From<ProviderError> for WalletAbiError {
    fn from(error: ProviderError) -> Self {
        WalletAbiError::InvalidRequest(error.to_string())
    }
}

struct SigningKeyStore(SwSigner);

impl KeyStoreMeta for SigningKeyStore {
    type Error = ProviderError;

    fn get_raw_signing_x_only_pubkey(&self) -> Result<XOnlyPublicKey, Self::Error> {
        Ok(self.0.xpub().public_key.x_only_public_key().0)
    }

    fn sign_pst(&self, pst: &mut PartiallySignedTransaction) -> Result<(), Self::Error> {
        self.0.sign(pst)?;
        Ok(())
    }

    fn sign_schnorr(
        &self,
        _message: Message,
        _xonly_public_key: XOnlyPublicKey,
    ) -> Result<Signature, Self::Error> {
        Err(ProviderError::Test)
    }
}

#[derive(Clone)]
struct FixedSessionFactory {
    session: WalletRequestSession,
}

impl WalletSessionFactory for FixedSessionFactory {
    type Error = ProviderError;

    async fn open_wallet_request_session(&self) -> Result<WalletRequestSession, Self::Error> {
        Ok(self.session.clone())
    }
}

struct TestPrevoutResolver {
    derivation_pair: (PublicKey, KeySource),
}

impl WalletPrevoutResolver for TestPrevoutResolver {
    type Error = ProviderError;

    fn get_bip32_derivation_pair(
        &self,
        _out_point: &OutPoint,
    ) -> Result<Option<(PublicKey, KeySource)>, Self::Error> {
        Ok(Some(self.derivation_pair.clone()))
    }

    fn unblind(&self, _tx_out: &TxOut) -> Result<TxOutSecrets, Self::Error> {
        Err(ProviderError::Test)
    }

    async fn get_tx_out(&self, _outpoint: OutPoint) -> Result<TxOut, Self::Error> {
        Err(ProviderError::Test)
    }
}

struct TestOutputAllocator {
    template: WalletOutputTemplate,
}

impl WalletOutputAllocator for TestOutputAllocator {
    type Error = ProviderError;

    fn get_wallet_output_template(
        &self,
        _session: &WalletRequestSession,
        _request: &WalletOutputRequest,
    ) -> Result<WalletOutputTemplate, Self::Error> {
        Ok(self.template.clone())
    }
}

struct TestBroadcaster {
    broadcast_calls: Arc<AtomicUsize>,
}

impl WalletBroadcaster for TestBroadcaster {
    type Error = ProviderError;

    fn broadcast_transaction(
        &self,
        _tx: &Transaction,
    ) -> impl Future<Output = Result<Txid, Self::Error>> + Send + '_ {
        self.broadcast_calls.fetch_add(1, Ordering::Relaxed);
        std::future::ready(Err(ProviderError::Test))
    }
}

struct TestReceiveAddressProvider {
    address: Address,
}

impl WalletReceiveAddressProvider for TestReceiveAddressProvider {
    type Error = ProviderError;

    fn get_signer_receive_address(&self) -> Result<Address, Self::Error> {
        Ok(self.address.clone())
    }
}

type TestProvider = WalletAbiProvider<
    SigningKeyStore,
    FixedSessionFactory,
    TestPrevoutResolver,
    TestOutputAllocator,
    TestBroadcaster,
    TestReceiveAddressProvider,
>;

struct TestHarness {
    provider: TestProvider,
    broadcast_calls: Arc<AtomicUsize>,
    policy_asset: AssetId,
}

fn build_provider() -> TestHarness {
    let signer = SwSigner::new(
        "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about",
        false,
    )
    .expect("signer");
    let derivation_path = DerivationPath::from_str("m/84h/1h/0h/0/0").expect("path");
    let xpub = signer.derive_xpub(&derivation_path).expect("xpub");
    let wallet_pubkey = PublicKey::new(xpub.public_key);
    let blinding_pubkey = BlindingPublicKey::from_str(
        "0279be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798",
    )
    .expect("blinding pubkey");
    let wallet_address = Address::p2wpkh(
        &wallet_pubkey,
        Some(blinding_pubkey),
        lwk_wollet::ElementsNetwork::LiquidTestnet.address_params(),
    );
    let policy_asset = lwk_wollet::ElementsNetwork::LiquidTestnet.policy_asset();
    let broadcast_calls = Arc::new(AtomicUsize::new(0));

    let provider = WalletAbiProviderBuilder::new(
        SigningKeyStore(signer),
        FixedSessionFactory {
            session: WalletRequestSession {
                session_id: "integration-session".to_string(),
                network: lwk_wollet::ElementsNetwork::LiquidTestnet,
                spendable_utxos: Arc::from(vec![ExternalUtxo {
                    outpoint: OutPoint::new(
                        Txid::from_str(
                            "0000000000000000000000000000000000000000000000000000000000000001",
                        )
                        .expect("txid"),
                        0,
                    ),
                    txout: TxOut {
                        asset: ConfidentialAsset::Explicit(policy_asset),
                        value: Value::Explicit(20_000),
                        nonce: Nonce::Null,
                        script_pubkey: wallet_address.script_pubkey(),
                        witness: TxOutWitness::default(),
                    },
                    tx: None,
                    unblinded: TxOutSecrets::new(
                        policy_asset,
                        lwk_wollet::elements::confidential::AssetBlindingFactor::zero(),
                        20_000,
                        lwk_wollet::elements::confidential::ValueBlindingFactor::zero(),
                    ),
                    max_weight_to_satisfy: 107,
                }]),
            },
        },
        TestPrevoutResolver {
            derivation_pair: (
                wallet_pubkey,
                (
                    Fingerprint::from_str("73c5da0a").expect("fingerprint"),
                    derivation_path,
                ),
            ),
        },
        TestOutputAllocator {
            template: WalletOutputTemplate {
                script_pubkey: wallet_address.script_pubkey(),
                blinding_pubkey: Some(blinding_pubkey),
            },
        },
        TestBroadcaster {
            broadcast_calls: Arc::clone(&broadcast_calls),
        },
        TestReceiveAddressProvider {
            address: wallet_address,
        },
    )
    .build();

    TestHarness {
        provider,
        broadcast_calls,
        policy_asset,
    }
}

fn runtime_params(policy_asset: AssetId) -> RuntimeParams {
    RuntimeParams {
        inputs: vec![],
        outputs: vec![OutputSchema {
            id: "external".to_string(),
            amount_sat: 5_000,
            lock: LockVariant::Script {
                script: Address::from_str(
                    "tlq1qq02egjncr8g4qn890mrw3jhgupwqymekv383lwpmsfghn36hac5ptpmeewtnftluqyaraa56ung7wf47crkn5fjuhk422d68m",
                )
                .expect("external address")
                .script_pubkey(),
            },
            asset: AssetVariant::AssetId { asset_id: policy_asset },
            blinder: BlinderVariant::Explicit,
        }],
        fee_rate_sat_kvb: Some(0.0),
        lock_time: None,
    }
}

#[test]
fn wallet_abi_provider_smoke() {
    let harness = build_provider();
    let provider = &harness.provider;

    let xonly = provider
        .get_raw_signing_x_only_pubkey()
        .expect("typed xonly")
        .to_string();
    assert_eq!(
        ready(provider.dispatch_json(
            GET_RAW_SIGNING_X_ONLY_PUBKEY_METHOD,
            serde_json::Value::Null,
        ))
        .expect("dispatch xonly"),
        serde_json::json!(xonly),
    );

    let receive_address = provider
        .get_signer_receive_address()
        .expect("typed receive address")
        .to_string();
    assert_eq!(
        ready(provider.dispatch_json(GET_SIGNER_RECEIVE_ADDRESS_METHOD, serde_json::Value::Null,))
            .expect("dispatch receive address"),
        serde_json::json!(receive_address),
    );

    let capabilities = ready(provider.get_capabilities()).expect("typed capabilities");
    let dispatch_capabilities: WalletCapabilities = dispatch(
        provider,
        WALLET_ABI_GET_CAPABILITIES_METHOD,
        serde_json::Value::Null,
    );
    assert_eq!(dispatch_capabilities, capabilities);

    let evaluate_request = TxEvaluateRequest::from_parts(
        "0d6d53cd-a040-4f0c-8d28-c67b6608fb14",
        lwk_wollet::ElementsNetwork::LiquidTestnet,
        runtime_params(harness.policy_asset),
    )
    .expect("evaluate request");
    let typed_evaluate =
        ready(provider.evaluate_request(evaluate_request.clone())).expect("typed evaluate request");
    let dispatch_evaluate: TxEvaluateResponse = dispatch(
        provider,
        WALLET_ABI_EVALUATE_REQUEST_METHOD,
        serde_json::to_value(&evaluate_request).expect("serialize evaluate request"),
    );
    assert_eq!(dispatch_evaluate, typed_evaluate);

    let process_request = TxCreateRequest::from_parts(
        "7a2f3ef0-6d1f-4ed4-b02d-a385684f6f21",
        lwk_wollet::ElementsNetwork::LiquidTestnet,
        runtime_params(harness.policy_asset),
        false,
    )
    .expect("process request");
    let typed_process =
        ready(provider.process_request(process_request.clone())).expect("typed process request");
    let dispatch_process: TxCreateResponse = dispatch(
        provider,
        WALLET_ABI_PROCESS_REQUEST_METHOD,
        serde_json::to_value(&process_request).expect("serialize process request"),
    );

    assert_eq!(dispatch_process.request_id, typed_process.request_id);
    assert_eq!(dispatch_process.network, typed_process.network);
    assert_eq!(dispatch_process.status, typed_process.status);
    let dispatch_preview = dispatch_process
        .preview()
        .expect("dispatch preview accessor");
    let typed_preview = typed_process.preview().expect("typed preview accessor");
    assert_eq!(dispatch_preview, typed_preview);
    assert_eq!(
        dispatch_process.transaction.is_some(),
        typed_process.transaction.is_some(),
    );
    assert_eq!(harness.broadcast_calls.load(Ordering::Relaxed), 0);
    let typed_preview = typed_preview.expect("typed preview");
    assert_eq!(
        typed_preview.asset_deltas,
        vec![PreviewAssetDelta {
            asset_id: harness.policy_asset,
            wallet_delta_sat: -5_000,
        }],
    );
    assert!(typed_preview.warnings.is_empty());
}

fn dispatch<T: DeserializeOwned>(
    provider: &TestProvider,
    method: &str,
    params: serde_json::Value,
) -> T {
    serde_json::from_value(ready(provider.dispatch_json(method, params)).expect("dispatch value"))
        .expect("deserialize dispatch value")
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
