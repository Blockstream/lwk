use crate::error::WalletAbiError;
use crate::wallet_abi::schema::runtime_deps::SplitWalletProvider;
use crate::wallet_abi::schema::{
    KeyStoreMeta, TxCreateRequest, TxCreateResponse, TxEvaluateRequest, TxEvaluateResponse,
    WalletBroadcaster, WalletCapabilities, WalletOutputAllocator, WalletPrevoutResolver,
    WalletReceiveAddressProvider, WalletRuntimeDeps, WalletSessionFactory,
};

use lwk_wollet::elements::Address;
use lwk_wollet::secp256k1::XOnlyPublicKey;

/// Checked-in wallet-abi provider façade built on top of the runtime engine.
pub struct WalletAbiProvider<
    Signer,
    SessionFactory,
    PrevoutResolver,
    OutputAllocator,
    Broadcaster,
    ReceiveAddressProvider,
> {
    signer_meta: Signer,
    wallet_deps: WalletRuntimeDeps<
        SessionFactory,
        SplitWalletProvider<PrevoutResolver, OutputAllocator, Broadcaster>,
    >,
    receive_address_provider: ReceiveAddressProvider,
}

/// Constructor helper for assembling a source-owned wallet-abi provider façade.
pub struct WalletAbiProviderBuilder<
    Signer,
    SessionFactory,
    PrevoutResolver,
    OutputAllocator,
    Broadcaster,
    ReceiveAddressProvider,
> {
    signer_meta: Signer,
    session_factory: SessionFactory,
    prevout_resolver: PrevoutResolver,
    output_allocator: OutputAllocator,
    broadcaster: Broadcaster,
    receive_address_provider: ReceiveAddressProvider,
}

impl<
        Signer,
        SessionFactory,
        PrevoutResolver,
        OutputAllocator,
        Broadcaster,
        ReceiveAddressProvider,
    >
    WalletAbiProviderBuilder<
        Signer,
        SessionFactory,
        PrevoutResolver,
        OutputAllocator,
        Broadcaster,
        ReceiveAddressProvider,
    >
{
    /// Capture the split provider roles and signer/session dependencies for one provider instance.
    pub fn new(
        signer_meta: Signer,
        session_factory: SessionFactory,
        prevout_resolver: PrevoutResolver,
        output_allocator: OutputAllocator,
        broadcaster: Broadcaster,
        receive_address_provider: ReceiveAddressProvider,
    ) -> Self {
        Self {
            signer_meta,
            session_factory,
            prevout_resolver,
            output_allocator,
            broadcaster,
            receive_address_provider,
        }
    }

    /// Assemble the provider façade from the captured runtime dependencies.
    pub fn build(
        self,
    ) -> WalletAbiProvider<
        Signer,
        SessionFactory,
        PrevoutResolver,
        OutputAllocator,
        Broadcaster,
        ReceiveAddressProvider,
    > {
        WalletAbiProvider {
            signer_meta: self.signer_meta,
            wallet_deps: WalletRuntimeDeps::new(
                self.session_factory,
                SplitWalletProvider::new(
                    self.prevout_resolver,
                    self.output_allocator,
                    self.broadcaster,
                ),
            ),
            receive_address_provider: self.receive_address_provider,
        }
    }
}

impl<
        Signer,
        SessionFactory,
        PrevoutResolver,
        OutputAllocator,
        Broadcaster,
        ReceiveAddressProvider,
    >
    WalletAbiProvider<
        Signer,
        SessionFactory,
        PrevoutResolver,
        OutputAllocator,
        Broadcaster,
        ReceiveAddressProvider,
    >
where
    Signer: KeyStoreMeta,
    WalletAbiError: From<Signer::Error>,
{
    /// Return the signer x-only public key exposed at provider connect time.
    pub fn get_raw_signing_x_only_pubkey(&self) -> Result<XOnlyPublicKey, WalletAbiError> {
        self.signer_meta
            .get_raw_signing_x_only_pubkey()
            .map_err(WalletAbiError::from)
    }
}

impl<
        Signer,
        SessionFactory,
        PrevoutResolver,
        OutputAllocator,
        Broadcaster,
        ReceiveAddressProvider,
    >
    WalletAbiProvider<
        Signer,
        SessionFactory,
        PrevoutResolver,
        OutputAllocator,
        Broadcaster,
        ReceiveAddressProvider,
    >
where
    ReceiveAddressProvider: WalletReceiveAddressProvider,
    WalletAbiError: From<ReceiveAddressProvider::Error>,
{
    /// Return the active wallet receive address exposed at provider connect time.
    pub fn get_signer_receive_address(&self) -> Result<Address, WalletAbiError> {
        self.receive_address_provider
            .get_signer_receive_address()
            .map_err(WalletAbiError::from)
    }
}

impl<
        Signer,
        SessionFactory,
        PrevoutResolver,
        OutputAllocator,
        Broadcaster,
        ReceiveAddressProvider,
    >
    WalletAbiProvider<
        Signer,
        SessionFactory,
        PrevoutResolver,
        OutputAllocator,
        Broadcaster,
        ReceiveAddressProvider,
    >
where
    Signer: KeyStoreMeta,
    SessionFactory: WalletSessionFactory,
    PrevoutResolver: WalletPrevoutResolver,
    OutputAllocator: WalletOutputAllocator<Error = PrevoutResolver::Error>,
    Broadcaster: WalletBroadcaster<Error = PrevoutResolver::Error>,
    WalletAbiError: From<Signer::Error> + From<SessionFactory::Error> + From<PrevoutResolver::Error>,
{
    /// Route one typed tx-create request through the checked-in runtime façade.
    pub async fn process_request(
        &self,
        request: TxCreateRequest,
    ) -> Result<TxCreateResponse, WalletAbiError> {
        crate::wallet_abi::WalletAbiRuntime::<TxCreateRequest, _, _, _>::new(
            request,
            &self.signer_meta,
            &self.wallet_deps,
        )
        .process_request()
        .await
    }

    /// Route one typed tx-evaluate request through the checked-in runtime façade.
    pub async fn evaluate_request(
        &self,
        request: TxEvaluateRequest,
    ) -> Result<TxEvaluateResponse, WalletAbiError> {
        crate::wallet_abi::WalletAbiRuntime::<TxEvaluateRequest, _, _, _>::new(
            request,
            &self.signer_meta,
            &self.wallet_deps,
        )
        .evaluate_request()
        .await
    }
}

impl<
        Signer,
        SessionFactory,
        PrevoutResolver,
        OutputAllocator,
        Broadcaster,
        ReceiveAddressProvider,
    >
    WalletAbiProvider<
        Signer,
        SessionFactory,
        PrevoutResolver,
        OutputAllocator,
        Broadcaster,
        ReceiveAddressProvider,
    >
where
    SessionFactory: WalletSessionFactory,
    WalletAbiError: From<SessionFactory::Error>,
{
    /// Return the provider discovery document for the active wallet/network context.
    pub async fn get_capabilities(&self) -> Result<WalletCapabilities, WalletAbiError> {
        let session = self
            .wallet_deps
            .session_factory
            .open_wallet_request_session()
            .await?;

        Ok(WalletCapabilities::new(
            session.network,
            [
                "get_signer_receive_address",
                "get_raw_signing_x_only_pubkey",
                "wallet_abi_evaluate_request",
                "wallet_abi_get_capabilities",
                "wallet_abi_process_request",
            ],
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::{Address, WalletAbiProviderBuilder, XOnlyPublicKey};
    use crate::error::WalletAbiError;
    use crate::wallet_abi::schema::{
        AssetFilter, AssetVariant, BlinderVariant, InputSchema, InputUnblinding, KeyStoreMeta,
        LockFilter, LockVariant, OutputSchema, TxCreateRequest, TxEvaluateRequest,
        WalletCapabilities, WalletOutputRequest, WalletBroadcaster, WalletOutputAllocator,
        WalletOutputTemplate, WalletPrevoutResolver, WalletReceiveAddressProvider,
        WalletRequestSession, WalletSessionFactory, WalletSourceFilter,
    };

    use lwk_common::Signer as _;
    use lwk_signer::SwSigner;
    use lwk_wollet::bitcoin::bip32::{DerivationPath, Fingerprint, KeySource};
    use lwk_wollet::bitcoin::PublicKey;
    use lwk_wollet::elements::confidential::{Asset as ConfidentialAsset, Nonce, Value};
    use lwk_wollet::elements::pset::PartiallySignedTransaction;
    use lwk_wollet::elements::secp256k1_zkp::PublicKey as BlindingPublicKey;
    use lwk_wollet::elements::{OutPoint, Transaction, TxOut, TxOutSecrets, TxOutWitness, Txid};
    use lwk_wollet::secp256k1::schnorr::Signature;
    use lwk_wollet::secp256k1::{Message, XOnlyPublicKey as SecpXOnlyPublicKey};
    use lwk_wollet::ExternalUtxo;

    use std::future::Future;
    use std::pin::pin;
    use std::str::FromStr;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::task::{Context, Poll, Waker};

    #[derive(Debug, thiserror::Error)]
    #[error("test provider error")]
    struct TestProviderError;

    impl From<TestProviderError> for WalletAbiError {
        fn from(error: TestProviderError) -> Self {
            WalletAbiError::InvalidRequest(error.to_string())
        }
    }

    struct TestSigner;
    struct TestSessionFactory;
    struct TestReceiveAddressProvider;

    impl KeyStoreMeta for TestSigner {
        type Error = TestProviderError;

        fn get_raw_signing_x_only_pubkey(&self) -> Result<SecpXOnlyPublicKey, Self::Error> {
            SecpXOnlyPublicKey::from_str(
                "79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798",
            )
            .map_err(|_| TestProviderError)
        }

        fn sign_pst(&self, _pst: &mut PartiallySignedTransaction) -> Result<(), Self::Error> {
            Ok(())
        }

        fn sign_schnorr(
            &self,
            _message: Message,
            _xonly_public_key: SecpXOnlyPublicKey,
        ) -> Result<Signature, Self::Error> {
            Err(TestProviderError)
        }
    }

    impl WalletSessionFactory for TestSessionFactory {
        type Error = TestProviderError;

        fn open_wallet_request_session(
            &self,
        ) -> impl Future<Output = Result<WalletRequestSession, Self::Error>> + Send + '_ {
            async move {
                Ok(WalletRequestSession {
                    session_id: "capabilities-session".to_string(),
                    network: lwk_wollet::ElementsNetwork::LiquidTestnet,
                    spendable_utxos: Arc::from(Vec::<ExternalUtxo>::new()),
                })
            }
        }
    }

    impl WalletReceiveAddressProvider for TestReceiveAddressProvider {
        type Error = TestProviderError;

        fn get_signer_receive_address(&self) -> Result<Address, Self::Error> {
            Address::from_str(
                "tlq1qq02egjncr8g4qn890mrw3jhgupwqymekv383lwpmsfghn36hac5ptpmeewtnftluqyaraa56ung7wf47crkn5fjuhk422d68m",
            )
            .map_err(|_| TestProviderError)
        }
    }

    #[test]
    fn provider_xonly_getter() {
        let provider = WalletAbiProviderBuilder::new(
            TestSigner,
            TestSessionFactory,
            TestPrevoutResolver {
                derivation_pair: (
                    PublicKey::from_str(
                        "0279be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798",
                    )
                    .expect("valid pubkey"),
                    (
                        Fingerprint::from_str("01020304").expect("valid fingerprint"),
                        DerivationPath::from_str("m/84h/1h/0h/0/0").expect("valid derivation path"),
                    ),
                ),
            },
            TestOutputAllocator {
                template: WalletOutputTemplate {
                    script_pubkey: Address::from_str(
                        "tlq1qq02egjncr8g4qn890mrw3jhgupwqymekv383lwpmsfghn36hac5ptpmeewtnftluqyaraa56ung7wf47crkn5fjuhk422d68m",
                    )
                    .expect("valid address")
                    .script_pubkey(),
                    blinding_pubkey: None,
                },
            },
            TestBroadcaster {
                broadcast_calls: Arc::new(AtomicUsize::new(0)),
            },
            TestReceiveAddressProvider,
        )
        .build();

        assert_eq!(
            provider
                .get_raw_signing_x_only_pubkey()
                .expect("xonly pubkey"),
            XOnlyPublicKey::from_str(
                "79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798",
            )
            .expect("valid xonly pubkey"),
        );
    }

    #[test]
    fn provider_receive_address_getter() {
        let provider = WalletAbiProviderBuilder::new(
            TestSigner,
            TestSessionFactory,
            TestPrevoutResolver {
                derivation_pair: (
                    PublicKey::from_str(
                        "0279be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798",
                    )
                    .expect("valid pubkey"),
                    (
                        Fingerprint::from_str("01020304").expect("valid fingerprint"),
                        DerivationPath::from_str("m/84h/1h/0h/0/0").expect("valid derivation path"),
                    ),
                ),
            },
            TestOutputAllocator {
                template: WalletOutputTemplate {
                    script_pubkey: Address::from_str(
                        "tlq1qq02egjncr8g4qn890mrw3jhgupwqymekv383lwpmsfghn36hac5ptpmeewtnftluqyaraa56ung7wf47crkn5fjuhk422d68m",
                    )
                    .expect("valid address")
                    .script_pubkey(),
                    blinding_pubkey: None,
                },
            },
            TestBroadcaster {
                broadcast_calls: Arc::new(AtomicUsize::new(0)),
            },
            TestReceiveAddressProvider,
        )
        .build();

        assert_eq!(
            provider
                .get_signer_receive_address()
                .expect("receive address")
                .to_string(),
            "tlq1qq02egjncr8g4qn890mrw3jhgupwqymekv383lwpmsfghn36hac5ptpmeewtnftluqyaraa56ung7wf47crkn5fjuhk422d68m",
        );
    }

    #[test]
    fn provider_capabilities_smoke() {
        let provider = WalletAbiProviderBuilder::new(
            TestSigner,
            TestSessionFactory,
            TestPrevoutResolver {
                derivation_pair: (
                    PublicKey::from_str(
                        "0279be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798",
                    )
                    .expect("valid pubkey"),
                    (
                        Fingerprint::from_str("01020304").expect("valid fingerprint"),
                        DerivationPath::from_str("m/84h/1h/0h/0/0").expect("valid derivation path"),
                    ),
                ),
            },
            TestOutputAllocator {
                template: WalletOutputTemplate {
                    script_pubkey: Address::from_str(
                        "tlq1qq02egjncr8g4qn890mrw3jhgupwqymekv383lwpmsfghn36hac5ptpmeewtnftluqyaraa56ung7wf47crkn5fjuhk422d68m",
                    )
                    .expect("valid address")
                    .script_pubkey(),
                    blinding_pubkey: None,
                },
            },
            TestBroadcaster {
                broadcast_calls: Arc::new(AtomicUsize::new(0)),
            },
            TestReceiveAddressProvider,
        )
        .build();

        assert_eq!(
            ready(provider.get_capabilities()).expect("provider capabilities"),
            WalletCapabilities::new(
                lwk_wollet::ElementsNetwork::LiquidTestnet,
                [
                    "get_signer_receive_address",
                    "get_raw_signing_x_only_pubkey",
                    "wallet_abi_evaluate_request",
                    "wallet_abi_get_capabilities",
                    "wallet_abi_process_request",
                ],
            ),
        );
    }

    #[derive(Debug, thiserror::Error)]
    enum ProcessRequestError {
        #[error(transparent)]
        Sign(#[from] lwk_signer::SignError),
        #[error("test process request error")]
        Test,
    }

    impl From<ProcessRequestError> for WalletAbiError {
        fn from(error: ProcessRequestError) -> Self {
            WalletAbiError::InvalidRequest(error.to_string())
        }
    }

    struct SigningKeyStore(SwSigner);

    impl KeyStoreMeta for SigningKeyStore {
        type Error = ProcessRequestError;

        fn get_raw_signing_x_only_pubkey(&self) -> Result<SecpXOnlyPublicKey, Self::Error> {
            Ok(self.0.xpub().public_key.x_only_public_key().0)
        }

        fn sign_pst(&self, pst: &mut PartiallySignedTransaction) -> Result<(), Self::Error> {
            self.0.sign(pst)?;
            Ok(())
        }

        fn sign_schnorr(
            &self,
            _message: Message,
            _xonly_public_key: SecpXOnlyPublicKey,
        ) -> Result<Signature, Self::Error> {
            Err(ProcessRequestError::Test)
        }
    }

    #[derive(Clone)]
    struct FixedSessionFactory {
        session: WalletRequestSession,
    }

    impl WalletSessionFactory for FixedSessionFactory {
        type Error = ProcessRequestError;

        fn open_wallet_request_session(
            &self,
        ) -> impl Future<Output = Result<WalletRequestSession, Self::Error>> + Send + '_ {
            let session = self.session.clone();
            async move { Ok(session) }
        }
    }

    struct TestPrevoutResolver {
        derivation_pair: (PublicKey, KeySource),
    }

    impl WalletPrevoutResolver for TestPrevoutResolver {
        type Error = ProcessRequestError;

        fn get_bip32_derivation_pair(
            &self,
            _out_point: &OutPoint,
        ) -> Result<Option<(PublicKey, KeySource)>, Self::Error> {
            Ok(Some(self.derivation_pair.clone()))
        }

        fn unblind(&self, _tx_out: &TxOut) -> Result<TxOutSecrets, Self::Error> {
            Err(ProcessRequestError::Test)
        }

        fn get_tx_out(
            &self,
            _outpoint: OutPoint,
        ) -> impl Future<Output = Result<TxOut, Self::Error>> + Send + '_ {
            async move { Err(ProcessRequestError::Test) }
        }
    }

    struct TestOutputAllocator {
        template: WalletOutputTemplate,
    }

    impl WalletOutputAllocator for TestOutputAllocator {
        type Error = ProcessRequestError;

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
        type Error = ProcessRequestError;

        fn broadcast_transaction(
            &self,
            _tx: &Transaction,
        ) -> impl Future<Output = Result<Txid, Self::Error>> + Send + '_ {
            self.broadcast_calls.fetch_add(1, Ordering::Relaxed);
            async move { Err(ProcessRequestError::Test) }
        }
    }

    #[test]
    fn provider_process_request_smoke() {
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
                    session_id: "session-1".to_string(),
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
            TestReceiveAddressProvider,
        )
        .build();
        let response = ready(provider.process_request(
            TxCreateRequest::from_parts(
                "0d6d53cd-a040-4f0c-8d28-c67b6608fb14",
                lwk_wollet::ElementsNetwork::LiquidTestnet,
                crate::wallet_abi::schema::RuntimeParams {
                    inputs: vec![InputSchema {
                        id: "wallet-input".to_string(),
                        utxo_source: crate::wallet_abi::schema::UTXOSource::Wallet {
                            filter: WalletSourceFilter {
                                asset: AssetFilter::Exact {
                                    asset_id: policy_asset,
                                },
                                amount: crate::wallet_abi::schema::AmountFilter::Exact {
                                    amount_sat: 20_000,
                                },
                                lock: LockFilter::None,
                            },
                        },
                        unblinding: InputUnblinding::Wallet,
                        ..InputSchema::default()
                    }],
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
                        asset: AssetVariant::AssetId {
                            asset_id: policy_asset,
                        },
                        blinder: BlinderVariant::Explicit,
                    }],
                    fee_rate_sat_kvb: Some(0.0),
                    lock_time: None,
                },
                false,
            )
            .expect("request"),
        ))
        .expect("process request");

        assert_eq!(broadcast_calls.load(Ordering::Relaxed), 0);
        assert_eq!(response.status, crate::wallet_abi::schema::tx_create::Status::Ok);
        assert_eq!(
            response
                .preview()
                .expect("preview accessor")
                .expect("preview")
                .asset_deltas,
            vec![crate::wallet_abi::schema::PreviewAssetDelta {
                asset_id: policy_asset,
                wallet_delta_sat: -5_000,
            }]
        );
    }

    #[test]
    fn provider_evaluate_request_smoke() {
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
                    session_id: "session-1".to_string(),
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
            TestReceiveAddressProvider,
        )
        .build();
        let response = ready(provider.evaluate_request(
            TxEvaluateRequest::from_parts(
                "0d6d53cd-a040-4f0c-8d28-c67b6608fb14",
                lwk_wollet::ElementsNetwork::LiquidTestnet,
                crate::wallet_abi::schema::RuntimeParams {
                    inputs: vec![InputSchema {
                        id: "wallet-input".to_string(),
                        utxo_source: crate::wallet_abi::schema::UTXOSource::Wallet {
                            filter: WalletSourceFilter {
                                asset: AssetFilter::Exact {
                                    asset_id: policy_asset,
                                },
                                amount: crate::wallet_abi::schema::AmountFilter::Exact {
                                    amount_sat: 20_000,
                                },
                                lock: LockFilter::None,
                            },
                        },
                        unblinding: InputUnblinding::Wallet,
                        ..InputSchema::default()
                    }],
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
                        asset: AssetVariant::AssetId {
                            asset_id: policy_asset,
                        },
                        blinder: BlinderVariant::Explicit,
                    }],
                    fee_rate_sat_kvb: Some(0.0),
                    lock_time: None,
                },
            )
            .expect("request"),
        ))
        .expect("evaluate request");

        assert_eq!(broadcast_calls.load(Ordering::Relaxed), 0);
        assert_eq!(response.status, crate::wallet_abi::schema::tx_create::Status::Ok);
        assert_eq!(
            response.preview.expect("preview").asset_deltas,
            vec![crate::wallet_abi::schema::PreviewAssetDelta {
                asset_id: policy_asset,
                wallet_delta_sat: -5_000,
            }]
        );
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
}
