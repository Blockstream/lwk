use std::future::Future;
use std::sync::Arc;

use crate::wallet_abi::request_session::{session_from_runtime, session_to_runtime};
use crate::{
    Address, LwkError, SignerMetaLink, WalletAbiCapabilities, WalletAbiRequestSession,
    WalletAbiTxCreateRequest, WalletAbiTxCreateResponse, WalletAbiTxEvaluateRequest,
    WalletAbiTxEvaluateResponse, WalletBroadcasterLink, WalletOutputAllocatorLink,
    WalletPrevoutResolverLink, WalletRuntimeDepsLink, WalletSessionFactoryLink, XOnlyPublicKey,
};
use lwk_simplicity::wallet_abi::{
    KeyStoreMeta, TxCreateRequest as SimplicityTxCreateRequest,
    TxEvaluateRequest as SimplicityTxEvaluateRequest,
    WalletAbiRuntime as SimplicityWalletAbiRuntime,
    WalletBroadcaster as SimplicityWalletBroadcaster, WalletOutputAllocator, WalletOutputRequest,
    WalletOutputTemplate, WalletPrevoutResolver, WalletProviderMeta, WalletReceiveAddressProvider,
    WalletRequestSession as SimplicityWalletRequestSession,
    WalletRuntimeDeps as SimplicityWalletRuntimeDeps,
    WalletSessionFactory as SimplicityWalletSessionFactory, GET_RAW_SIGNING_X_ONLY_PUBKEY_METHOD,
    GET_SIGNER_RECEIVE_ADDRESS_METHOD, WALLET_ABI_EVALUATE_REQUEST_METHOD,
    WALLET_ABI_GET_CAPABILITIES_METHOD, WALLET_ABI_PROCESS_REQUEST_METHOD,
};
use lwk_wollet::bitcoin::bip32::KeySource;
use lwk_wollet::bitcoin::PublicKey;
use lwk_wollet::elements::pset::PartiallySignedTransaction;
use lwk_wollet::elements::{
    OutPoint as ElementsOutPoint, Transaction as ElementsTransaction, TxOut as ElementsTxOut,
    TxOutSecrets as ElementsTxOutSecrets, Txid as ElementsTxid,
};
use lwk_wollet::secp256k1::schnorr::Signature;
use lwk_wollet::secp256k1::{Message, XOnlyPublicKey as SecpXOnlyPublicKey};

#[derive(Debug, thiserror::Error)]
#[error("{0}")]
struct ProviderRuntimeError(String);

impl From<ProviderRuntimeError> for lwk_simplicity::error::WalletAbiError {
    fn from(error: ProviderRuntimeError) -> Self {
        lwk_simplicity::error::WalletAbiError::InvalidRequest(error.to_string())
    }
}

struct ProviderSigner {
    inner: Arc<SignerMetaLink>,
}

impl KeyStoreMeta for ProviderSigner {
    type Error = ProviderRuntimeError;

    fn get_raw_signing_x_only_pubkey(&self) -> Result<SecpXOnlyPublicKey, Self::Error> {
        self.inner
            .get_raw_signing_x_only_pubkey()
            .map_err(|error| ProviderRuntimeError(error.to_string()))
    }

    fn sign_pst(&self, pst: &mut PartiallySignedTransaction) -> Result<(), Self::Error> {
        self.inner
            .sign_pst(pst)
            .map_err(|error| ProviderRuntimeError(error.to_string()))
    }

    fn sign_schnorr(
        &self,
        message: Message,
        xonly_public_key: SecpXOnlyPublicKey,
    ) -> Result<Signature, Self::Error> {
        self.inner
            .sign_schnorr(message, xonly_public_key)
            .map_err(|error| ProviderRuntimeError(error.to_string()))
    }
}

struct ProviderSessionFactory {
    inner: Arc<WalletSessionFactoryLink>,
}

impl SimplicityWalletSessionFactory for ProviderSessionFactory {
    type Error = ProviderRuntimeError;

    async fn open_wallet_request_session(
        &self,
    ) -> Result<SimplicityWalletRequestSession, Self::Error> {
        self.inner
            .open_wallet_request_session()
            .await
            .map_err(|error| ProviderRuntimeError(error.to_string()))
    }
}

struct FixedProviderSessionFactory {
    session: SimplicityWalletRequestSession,
}

impl SimplicityWalletSessionFactory for FixedProviderSessionFactory {
    type Error = ProviderRuntimeError;

    async fn open_wallet_request_session(
        &self,
    ) -> Result<SimplicityWalletRequestSession, Self::Error> {
        Ok(self.session.clone())
    }
}

struct ProviderWalletMeta {
    prevout_resolver: Arc<WalletPrevoutResolverLink>,
    output_allocator: Arc<WalletOutputAllocatorLink>,
    broadcaster: Arc<WalletBroadcasterLink>,
}

impl WalletProviderMeta for ProviderWalletMeta {
    type Error = ProviderRuntimeError;

    fn get_bip32_derivation_pair(
        &self,
        out_point: &ElementsOutPoint,
    ) -> Result<Option<(PublicKey, KeySource)>, Self::Error> {
        self.prevout_resolver
            .get_bip32_derivation_pair(out_point)
            .map_err(|error| ProviderRuntimeError(error.to_string()))
    }

    fn unblind(&self, tx_out: &ElementsTxOut) -> Result<ElementsTxOutSecrets, Self::Error> {
        self.prevout_resolver
            .unblind(tx_out)
            .map_err(|error| ProviderRuntimeError(error.to_string()))
    }

    async fn get_tx_out(&self, outpoint: ElementsOutPoint) -> Result<ElementsTxOut, Self::Error> {
        self.prevout_resolver
            .get_tx_out(outpoint)
            .await
            .map_err(|error| ProviderRuntimeError(error.to_string()))
    }

    fn get_wallet_output_template(
        &self,
        session: &SimplicityWalletRequestSession,
        request: &WalletOutputRequest,
    ) -> Result<WalletOutputTemplate, Self::Error> {
        self.output_allocator
            .get_wallet_output_template(session, request)
            .map_err(|error| ProviderRuntimeError(error.to_string()))
    }

    fn broadcast_transaction(
        &self,
        tx: &ElementsTransaction,
    ) -> impl Future<Output = Result<ElementsTxid, Self::Error>> + Send + '_ {
        let tx = tx.clone();
        async move {
            self.broadcaster
                .broadcast_transaction(&tx)
                .await
                .map_err(|error| ProviderRuntimeError(error.to_string()))
        }
    }
}

/// Source-owned bindings wrapper for the checked-in Wallet ABI provider facade.
#[derive(uniffi::Object)]
pub struct WalletAbiProvider {
    signer: Arc<SignerMetaLink>,
    wallet: Arc<WalletRuntimeDepsLink>,
}

#[uniffi::export]
impl WalletAbiProvider {
    /// Create a provider object from a signer bridge and split wallet runtime dependencies.
    #[uniffi::constructor]
    pub fn new(signer: Arc<SignerMetaLink>, wallet: Arc<WalletRuntimeDepsLink>) -> Self {
        Self { signer, wallet }
    }

    /// Return the signer x-only public key exposed at provider connect time.
    pub fn get_raw_signing_x_only_pubkey(&self) -> Result<Arc<XOnlyPublicKey>, LwkError> {
        Ok(Arc::new(
            self.signer
                .get_raw_signing_x_only_pubkey()
                .map_err(|error| LwkError::from(format!("{error}")))?
                .into(),
        ))
    }

    /// Return the active wallet receive address exposed at provider connect time.
    pub fn get_signer_receive_address(&self) -> Result<Arc<Address>, LwkError> {
        Ok(Arc::new(
            self.wallet
                .receive_address_provider
                .get_signer_receive_address()
                .map_err(|error| LwkError::from(format!("{error}")))?
                .into(),
        ))
    }

    /// Capture one request-scoped wallet session snapshot for later reuse.
    pub fn capture_request_session(&self) -> Result<WalletAbiRequestSession, LwkError> {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|error| LwkError::from(error.to_string()))?;
        let session = runtime
            .block_on(self.wallet.session_factory.open_wallet_request_session())
            .map_err(|error| LwkError::from(error.to_string()))?;

        Ok(session_from_runtime(&session))
    }

    /// Route one typed tx-create request through the checked-in runtime facade.
    pub fn process_request(
        &self,
        request: &WalletAbiTxCreateRequest,
    ) -> Result<Arc<WalletAbiTxCreateResponse>, LwkError> {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|error| LwkError::from(error.to_string()))?;
        let signer = ProviderSigner {
            inner: Arc::clone(&self.signer),
        };
        let wallet_deps = SimplicityWalletRuntimeDeps::new(
            ProviderSessionFactory {
                inner: Arc::clone(&self.wallet.session_factory),
            },
            ProviderWalletMeta {
                prevout_resolver: Arc::clone(&self.wallet.prevout_resolver),
                output_allocator: Arc::clone(&self.wallet.output_allocator),
                broadcaster: Arc::clone(&self.wallet.broadcaster),
            },
        );

        Ok(Arc::new(WalletAbiTxCreateResponse {
            inner: runtime.block_on(
                SimplicityWalletAbiRuntime::<SimplicityTxCreateRequest, _, _, _>::new(
                    request.inner.clone(),
                    &signer,
                    &wallet_deps,
                )
                .process_request(),
            )?,
        }))
    }

    /// Route one typed tx-create request through the checked-in runtime facade using a frozen
    /// request session snapshot.
    pub fn process_request_with_session(
        &self,
        session: &WalletAbiRequestSession,
        request: &WalletAbiTxCreateRequest,
    ) -> Result<Arc<WalletAbiTxCreateResponse>, LwkError> {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|error| LwkError::from(error.to_string()))?;
        let signer = ProviderSigner {
            inner: Arc::clone(&self.signer),
        };
        let wallet_deps = SimplicityWalletRuntimeDeps::new(
            FixedProviderSessionFactory {
                session: session_to_runtime(session),
            },
            ProviderWalletMeta {
                prevout_resolver: Arc::clone(&self.wallet.prevout_resolver),
                output_allocator: Arc::clone(&self.wallet.output_allocator),
                broadcaster: Arc::clone(&self.wallet.broadcaster),
            },
        );

        Ok(Arc::new(WalletAbiTxCreateResponse {
            inner: runtime.block_on(
                SimplicityWalletAbiRuntime::<SimplicityTxCreateRequest, _, _, _>::new(
                    request.inner.clone(),
                    &signer,
                    &wallet_deps,
                )
                .process_request(),
            )?,
        }))
    }

    /// Return the provider discovery document for the active wallet/network context.
    pub fn get_capabilities(&self) -> Result<Arc<WalletAbiCapabilities>, LwkError> {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|error| LwkError::from(error.to_string()))?;
        let session = runtime
            .block_on(self.wallet.session_factory.open_wallet_request_session())
            .map_err(|error| LwkError::from(error.to_string()))?;

        Ok(Arc::new(WalletAbiCapabilities {
            inner: lwk_simplicity::wallet_abi::WalletCapabilities::new(
                session.network,
                [
                    GET_SIGNER_RECEIVE_ADDRESS_METHOD,
                    GET_RAW_SIGNING_X_ONLY_PUBKEY_METHOD,
                    WALLET_ABI_EVALUATE_REQUEST_METHOD,
                    WALLET_ABI_GET_CAPABILITIES_METHOD,
                    WALLET_ABI_PROCESS_REQUEST_METHOD,
                ],
            ),
        }))
    }

    /// Route one typed tx-evaluate request through the checked-in runtime facade.
    pub fn evaluate_request(
        &self,
        request: &WalletAbiTxEvaluateRequest,
    ) -> Result<Arc<WalletAbiTxEvaluateResponse>, LwkError> {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|error| LwkError::from(error.to_string()))?;
        let signer = ProviderSigner {
            inner: Arc::clone(&self.signer),
        };
        let wallet_deps = SimplicityWalletRuntimeDeps::new(
            ProviderSessionFactory {
                inner: Arc::clone(&self.wallet.session_factory),
            },
            ProviderWalletMeta {
                prevout_resolver: Arc::clone(&self.wallet.prevout_resolver),
                output_allocator: Arc::clone(&self.wallet.output_allocator),
                broadcaster: Arc::clone(&self.wallet.broadcaster),
            },
        );

        Ok(Arc::new(WalletAbiTxEvaluateResponse {
            inner: runtime.block_on(
                SimplicityWalletAbiRuntime::<SimplicityTxEvaluateRequest, _, _, _>::new(
                    request.inner.clone(),
                    &signer,
                    &wallet_deps,
                )
                .evaluate_request(),
            )?,
        }))
    }

    /// Route one typed tx-evaluate request through the checked-in runtime facade using a frozen
    /// request session snapshot.
    pub fn evaluate_request_with_session(
        &self,
        session: &WalletAbiRequestSession,
        request: &WalletAbiTxEvaluateRequest,
    ) -> Result<Arc<WalletAbiTxEvaluateResponse>, LwkError> {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|error| LwkError::from(error.to_string()))?;
        let signer = ProviderSigner {
            inner: Arc::clone(&self.signer),
        };
        let wallet_deps = SimplicityWalletRuntimeDeps::new(
            FixedProviderSessionFactory {
                session: session_to_runtime(session),
            },
            ProviderWalletMeta {
                prevout_resolver: Arc::clone(&self.wallet.prevout_resolver),
                output_allocator: Arc::clone(&self.wallet.output_allocator),
                broadcaster: Arc::clone(&self.wallet.broadcaster),
            },
        );

        Ok(Arc::new(WalletAbiTxEvaluateResponse {
            inner: runtime.block_on(
                SimplicityWalletAbiRuntime::<SimplicityTxEvaluateRequest, _, _, _>::new(
                    request.inner.clone(),
                    &signer,
                    &wallet_deps,
                )
                .evaluate_request(),
            )?,
        }))
    }

    /// Dispatch one method-level JSON call without owning the outer JSON-RPC envelope.
    pub fn dispatch_json(&self, method: &str, params_json: &str) -> Result<String, LwkError> {
        match method {
            GET_SIGNER_RECEIVE_ADDRESS_METHOD => {
                expect_no_params_json(method, params_json)?;
                Ok(serde_json::to_string(
                    &self.get_signer_receive_address()?.to_string(),
                )?)
            }
            GET_RAW_SIGNING_X_ONLY_PUBKEY_METHOD => {
                expect_no_params_json(method, params_json)?;
                Ok(serde_json::to_string(
                    &self.get_raw_signing_x_only_pubkey()?.to_string(),
                )?)
            }
            WALLET_ABI_GET_CAPABILITIES_METHOD => {
                expect_no_params_json(method, params_json)?;
                self.get_capabilities()?.to_json()
            }
            WALLET_ABI_PROCESS_REQUEST_METHOD => {
                let request = WalletAbiTxCreateRequest::from_json(params_json)?;
                self.process_request(request.as_ref())
                    .and_then(|response| response.to_json())
            }
            WALLET_ABI_EVALUATE_REQUEST_METHOD => {
                let request = WalletAbiTxEvaluateRequest::from_json(params_json)?;
                self.evaluate_request(request.as_ref())
                    .and_then(|response| response.to_json())
            }
            _ => Err(LwkError::from(format!(
                "unsupported wallet-abi method '{method}'"
            ))),
        }
    }
}

fn expect_no_params_json(method: &str, params_json: &str) -> Result<(), LwkError> {
    if serde_json::from_str::<serde_json::Value>(params_json)?.is_null() {
        return Ok(());
    }

    Err(LwkError::from(format!(
        "wallet-abi method '{method}' does not accept params"
    )))
}

#[cfg(test)]
mod frozen_session_tests {
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Arc, Mutex};

    use super::WalletAbiProvider;
    use crate::{
        wallet_abi_bip32_derivation_pair_from_signer, wallet_abi_output_template_from_address,
        Address, ExternalUtxo, LwkError, Mnemonic, Network, OutPoint, Pset, Signer,
        SignerMetaLink, Transaction, TxOut, TxOutSecrets, TxSequence, WalletAbiAmountFilter,
        WalletAbiAssetFilter, WalletAbiAssetVariant, WalletAbiBip32DerivationPair,
        WalletAbiBlinderVariant, WalletAbiBroadcasterCallbacks, WalletAbiFinalizerSpec,
        WalletAbiInputSchema, WalletAbiInputUnblinding, WalletAbiLockFilter,
        WalletAbiLockVariant, WalletAbiOutputAllocatorCallbacks, WalletAbiOutputSchema,
        WalletAbiPrevoutResolverCallbacks, WalletAbiReceiveAddressProviderCallbacks,
        WalletAbiRequestSession, WalletAbiRuntimeParams, WalletAbiSessionFactoryCallbacks,
        WalletAbiSignerCallbacks, WalletAbiTxCreateRequest, WalletAbiTxEvaluateRequest,
        WalletAbiUtxoSource, WalletAbiWalletOutputRequest, WalletAbiWalletOutputRole,
        WalletAbiWalletOutputTemplate, WalletAbiWalletSourceFilter, WalletBroadcasterLink,
        WalletOutputAllocatorLink, WalletPrevoutResolverLink, WalletReceiveAddressProviderLink,
        WalletRuntimeDepsLink, WalletSessionFactoryLink, XOnlyPublicKey,
    };

    struct TestSignerCallbacks {
        signer: Arc<Signer>,
        expected_xonly: Arc<XOnlyPublicKey>,
    }

    impl WalletAbiSignerCallbacks for TestSignerCallbacks {
        fn get_raw_signing_x_only_pubkey(&self) -> Result<Arc<XOnlyPublicKey>, LwkError> {
            Ok(self.expected_xonly.clone())
        }

        fn sign_pst(&self, pst: Arc<Pset>) -> Result<Arc<Pset>, LwkError> {
            self.signer.sign(pst.as_ref())
        }

        fn sign_schnorr(&self, _message: Vec<u8>) -> Result<Vec<u8>, LwkError> {
            Ok(vec![0; 64])
        }
    }

    struct TestSessionFactoryCallbacks {
        open_calls: Arc<AtomicUsize>,
        session: WalletAbiRequestSession,
    }

    impl WalletAbiSessionFactoryCallbacks for TestSessionFactoryCallbacks {
        fn open_wallet_request_session(&self) -> Result<WalletAbiRequestSession, LwkError> {
            self.open_calls.fetch_add(1, Ordering::SeqCst);
            Ok(self.session.clone())
        }
    }

    struct TestOutputAllocatorCallbacks {
        session_ids: Arc<Mutex<Vec<String>>>,
        address: Arc<Address>,
    }

    impl WalletAbiOutputAllocatorCallbacks for TestOutputAllocatorCallbacks {
        fn get_wallet_output_template(
            &self,
            session: WalletAbiRequestSession,
            request: WalletAbiWalletOutputRequest,
        ) -> Result<WalletAbiWalletOutputTemplate, LwkError> {
            self.session_ids.lock()?.push(session.session_id);
            assert!(matches!(
                request.role,
                WalletAbiWalletOutputRole::Receive | WalletAbiWalletOutputRole::Change
            ));
            Ok(wallet_abi_output_template_from_address(
                self.address.as_ref(),
            ))
        }
    }

    struct TestPrevoutResolverCallbacks {
        derivation_pair: WalletAbiBip32DerivationPair,
        tx_out: Arc<TxOut>,
        tx_out_secrets: Arc<TxOutSecrets>,
    }

    impl WalletAbiPrevoutResolverCallbacks for TestPrevoutResolverCallbacks {
        fn get_bip32_derivation_pair(
            &self,
            _outpoint: Arc<OutPoint>,
        ) -> Result<Option<WalletAbiBip32DerivationPair>, LwkError> {
            Ok(Some(self.derivation_pair.clone()))
        }

        fn unblind(&self, _tx_out: Arc<TxOut>) -> Result<Arc<TxOutSecrets>, LwkError> {
            Ok(self.tx_out_secrets.clone())
        }

        fn get_tx_out(&self, _outpoint: Arc<OutPoint>) -> Result<Arc<TxOut>, LwkError> {
            Ok(self.tx_out.clone())
        }
    }

    struct TestBroadcasterCallbacks {
        broadcast_calls: Arc<AtomicUsize>,
    }

    impl WalletAbiBroadcasterCallbacks for TestBroadcasterCallbacks {
        fn broadcast_transaction(
            &self,
            tx: Arc<Transaction>,
        ) -> Result<Arc<crate::Txid>, LwkError> {
            self.broadcast_calls.fetch_add(1, Ordering::SeqCst);
            Ok(tx.txid())
        }
    }

    struct TestReceiveAddressProviderCallbacks {
        address: Arc<Address>,
    }

    impl WalletAbiReceiveAddressProviderCallbacks for TestReceiveAddressProviderCallbacks {
        fn get_signer_receive_address(&self) -> Result<Arc<Address>, LwkError> {
            Ok(self.address.clone())
        }
    }

    #[test]
    fn wallet_abi_provider_reuses_frozen_request_session() {
        let network = Network::testnet();
        let mnemonic = Mnemonic::new(
            "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about",
        )
        .expect("mnemonic");
        let signer = Signer::new(&mnemonic, network.as_ref()).expect("signer");
        let wallet_descriptor = signer.wpkh_slip77_descriptor().expect("descriptor");
        let wallet =
            crate::Wollet::new(network.as_ref(), wallet_descriptor.as_ref(), None).expect("wallet");
        let wallet_address = wallet.address(Some(0)).expect("address result").address();
        let external_address = Address::new(
            "tlq1qq02egjncr8g4qn890mrw3jhgupwqymekv383lwpmsfghn36hac5ptpmeewtnftluqyaraa56ung7wf47crkn5fjuhk422d68m",
        )
        .expect("external address");
        let expected_xonly =
            crate::simplicity_derive_xonly_pubkey(signer.as_ref(), "m/86h/1h/0h/0/0")
                .expect("xonly");
        let derivation_pair = wallet_abi_bip32_derivation_pair_from_signer(
            signer.as_ref(),
            vec![2147483732, 2147483649, 2147483648, 0, 0],
        )
        .expect("derivation pair");
        let policy_asset = network.policy_asset();
        let outpoint = OutPoint::from_parts(
            &crate::Txid::from_string(
                "0000000000000000000000000000000000000000000000000000000000000001",
            )
            .expect("txid"),
            0,
        );
        let tx_out =
            TxOut::from_explicit(wallet_address.script_pubkey().as_ref(), policy_asset, 20_000);
        let tx_out_secrets = TxOutSecrets::from_explicit(policy_asset, 20_000);
        let utxo = ExternalUtxo::from_unchecked_data(&outpoint, &tx_out, &tx_out_secrets, 107);
        let request_session = WalletAbiRequestSession {
            session_id: "frozen-session".to_owned(),
            network: network.clone(),
            spendable_utxos: vec![utxo],
        };

        let open_calls = Arc::new(AtomicUsize::new(0));
        let output_session_ids = Arc::new(Mutex::new(Vec::new()));
        let broadcast_calls = Arc::new(AtomicUsize::new(0));

        let signer_link = Arc::new(SignerMetaLink::new(Arc::new(TestSignerCallbacks {
            signer: signer.clone(),
            expected_xonly,
        })));
        let session_factory = Arc::new(WalletSessionFactoryLink::new(Arc::new(
            TestSessionFactoryCallbacks {
                open_calls: open_calls.clone(),
                session: request_session,
            },
        )));
        let output_allocator = Arc::new(WalletOutputAllocatorLink::new(Arc::new(
            TestOutputAllocatorCallbacks {
                session_ids: output_session_ids.clone(),
                address: wallet_address.clone(),
            },
        )));
        let prevout_resolver = Arc::new(WalletPrevoutResolverLink::new(Arc::new(
            TestPrevoutResolverCallbacks {
                derivation_pair,
                tx_out: tx_out.clone(),
                tx_out_secrets: tx_out_secrets.clone(),
            },
        )));
        let broadcaster = Arc::new(WalletBroadcasterLink::new(Arc::new(
            TestBroadcasterCallbacks {
                broadcast_calls: broadcast_calls.clone(),
            },
        )));
        let receive_address_provider = Arc::new(WalletReceiveAddressProviderLink::new(Arc::new(
            TestReceiveAddressProviderCallbacks {
                address: wallet_address.clone(),
            },
        )));
        let runtime_deps = Arc::new(WalletRuntimeDepsLink::new(
            session_factory,
            output_allocator,
            prevout_resolver,
            broadcaster,
            receive_address_provider,
        ));

        let provider = WalletAbiProvider::new(signer_link, runtime_deps);

        let asset_filter = WalletAbiAssetFilter::exact(policy_asset);
        let amount_filter = WalletAbiAmountFilter::exact(20_000);
        let lock_filter = WalletAbiLockFilter::none();
        let wallet_filter = WalletAbiWalletSourceFilter::with_filters(
            asset_filter.as_ref(),
            amount_filter.as_ref(),
            lock_filter.as_ref(),
        );
        let utxo_source = WalletAbiUtxoSource::wallet(wallet_filter.as_ref());
        let unblinding = WalletAbiInputUnblinding::wallet();
        let sequence = TxSequence::max();
        let finalizer = WalletAbiFinalizerSpec::wallet();
        let input = WalletAbiInputSchema::from_sequence(
            "wallet-input",
            utxo_source.as_ref(),
            unblinding.as_ref(),
            sequence.as_ref(),
            finalizer.as_ref(),
        );
        let lock_variant = WalletAbiLockVariant::script(external_address.script_pubkey().as_ref());
        let asset_variant = WalletAbiAssetVariant::asset_id(policy_asset);
        let blinder_variant = WalletAbiBlinderVariant::explicit();
        let output = WalletAbiOutputSchema::new(
            "external",
            5_000,
            lock_variant.as_ref(),
            asset_variant.as_ref(),
            blinder_variant.as_ref(),
        );
        let params = WalletAbiRuntimeParams::new(&[input], &[output], Some(0.0), None);
        let create_request = WalletAbiTxCreateRequest::from_parts(
            "0d6d53cd-a040-4f0c-8d28-c67b6608fb14",
            network.as_ref(),
            params.as_ref(),
            true,
        )
        .expect("request");
        let evaluate_request = WalletAbiTxEvaluateRequest::from_parts(
            &create_request.request_id(),
            create_request.network().as_ref(),
            create_request.params().as_ref(),
        )
        .expect("evaluate request");

        let session = provider
            .capture_request_session()
            .expect("capture request session");
        let evaluate_response = provider
            .evaluate_request_with_session(&session, evaluate_request.as_ref())
            .expect("evaluate request");
        let create_response = provider
            .process_request_with_session(&session, create_request.as_ref())
            .expect("create request");

        assert_eq!(open_calls.load(Ordering::SeqCst), 1);
        assert!(output_session_ids
            .lock()
            .expect("session ids")
            .iter()
            .all(|session_id| session_id == "frozen-session"));
        assert_eq!(
            evaluate_response
                .preview()
                .expect("preview")
                .to_json()
                .expect("preview json"),
            create_response
                .preview()
                .expect("preview result")
                .expect("preview")
                .to_json()
                .expect("preview json")
        );
        assert_eq!(broadcast_calls.load(Ordering::SeqCst), 1);
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use crate::{
        wallet_abi_bip32_derivation_pair_from_signer, Address, ExternalUtxo, LwkError, Mnemonic,
        Network, OutPoint, Pset, Signer, Transaction, TxOut, TxOutSecrets, TxSequence, Txid,
        WalletAbiAmountFilter, WalletAbiAssetFilter, WalletAbiAssetVariant,
        WalletAbiBlinderVariant, WalletAbiBroadcasterCallbacks, WalletAbiFinalizerSpec,
        WalletAbiInputSchema, WalletAbiInputUnblinding, WalletAbiLockFilter, WalletAbiLockVariant,
        WalletAbiOutputAllocatorCallbacks, WalletAbiPrevoutResolverCallbacks,
        WalletAbiReceiveAddressProviderCallbacks, WalletAbiRequestSession, WalletAbiRuntimeParams,
        WalletAbiSessionFactoryCallbacks, WalletAbiSignerCallbacks, WalletAbiSignerContext,
        WalletAbiTxCreateRequest, WalletAbiTxEvaluateRequest, WalletAbiUtxoSource,
        WalletAbiWalletOutputRequest, WalletAbiWalletOutputTemplate, WalletAbiWalletSourceFilter,
        WalletBroadcasterLink, WalletOutputAllocatorLink, WalletPrevoutResolverLink,
        WalletReceiveAddressProviderLink, WalletSessionFactoryLink,
    };
    use elements::bitcoin::secp256k1::{Keypair, Secp256k1, SecretKey};
    use std::str::FromStr;

    struct TestSignerCallbacks {
        keypair: Keypair,
    }

    impl WalletAbiSignerCallbacks for TestSignerCallbacks {
        fn get_raw_signing_x_only_pubkey(&self) -> Result<Arc<XOnlyPublicKey>, LwkError> {
            Ok(XOnlyPublicKey::from_keypair(&self.keypair))
        }

        fn sign_pst(&self, pst: Arc<Pset>) -> Result<Arc<Pset>, LwkError> {
            Ok(pst)
        }

        fn sign_schnorr(&self, _message: Vec<u8>) -> Result<Vec<u8>, LwkError> {
            unreachable!("not used in provider xonly test")
        }
    }

    struct TestSessionFactoryCallbacks;

    impl WalletAbiSessionFactoryCallbacks for TestSessionFactoryCallbacks {
        fn open_wallet_request_session(&self) -> Result<WalletAbiRequestSession, LwkError> {
            Ok(WalletAbiRequestSession {
                session_id: "capabilities-session".to_string(),
                network: Network::testnet(),
                spendable_utxos: vec![],
            })
        }
    }

    struct TestPrevoutResolverCallbacks;

    impl WalletAbiPrevoutResolverCallbacks for TestPrevoutResolverCallbacks {
        fn get_bip32_derivation_pair(
            &self,
            _outpoint: Arc<OutPoint>,
        ) -> Result<Option<crate::WalletAbiBip32DerivationPair>, LwkError> {
            unreachable!("not used in provider xonly test")
        }

        fn unblind(&self, _tx_out: Arc<TxOut>) -> Result<Arc<TxOutSecrets>, LwkError> {
            unreachable!("not used in provider xonly test")
        }

        fn get_tx_out(&self, _outpoint: Arc<OutPoint>) -> Result<Arc<TxOut>, LwkError> {
            unreachable!("not used in provider xonly test")
        }
    }

    struct TestOutputAllocatorCallbacks;

    impl WalletAbiOutputAllocatorCallbacks for TestOutputAllocatorCallbacks {
        fn get_wallet_output_template(
            &self,
            _session: WalletAbiRequestSession,
            _request: WalletAbiWalletOutputRequest,
        ) -> Result<WalletAbiWalletOutputTemplate, LwkError> {
            unreachable!("not used in provider xonly test")
        }
    }

    struct TestBroadcasterCallbacks;

    impl WalletAbiBroadcasterCallbacks for TestBroadcasterCallbacks {
        fn broadcast_transaction(&self, _tx: Arc<Transaction>) -> Result<Arc<Txid>, LwkError> {
            unreachable!("not used in provider xonly test")
        }
    }

    struct TestReceiveAddressProviderCallbacks;

    impl WalletAbiReceiveAddressProviderCallbacks for TestReceiveAddressProviderCallbacks {
        fn get_signer_receive_address(&self) -> Result<Arc<Address>, LwkError> {
            Address::new(
                "tlq1qq2xvpcvfup5j8zscjq05u2wxxjcyewk7979f3mmz5l7uw5pqmx6xf5xy50hsn6vhkm5euwt72x878eq6zxx2z58hd7zrsg9qn",
            )
        }
    }

    struct ProcessSessionFactoryCallbacks {
        session: WalletAbiRequestSession,
    }

    impl WalletAbiSessionFactoryCallbacks for ProcessSessionFactoryCallbacks {
        fn open_wallet_request_session(&self) -> Result<WalletAbiRequestSession, LwkError> {
            Ok(self.session.clone())
        }
    }

    struct ProcessPrevoutResolverCallbacks {
        derivation_pair: crate::WalletAbiBip32DerivationPair,
        tx_out: Arc<TxOut>,
        secrets: Arc<TxOutSecrets>,
    }

    impl WalletAbiPrevoutResolverCallbacks for ProcessPrevoutResolverCallbacks {
        fn get_bip32_derivation_pair(
            &self,
            _outpoint: Arc<OutPoint>,
        ) -> Result<Option<crate::WalletAbiBip32DerivationPair>, LwkError> {
            Ok(Some(self.derivation_pair.clone()))
        }

        fn unblind(&self, _tx_out: Arc<TxOut>) -> Result<Arc<TxOutSecrets>, LwkError> {
            Ok(self.secrets.clone())
        }

        fn get_tx_out(&self, _outpoint: Arc<OutPoint>) -> Result<Arc<TxOut>, LwkError> {
            Ok(self.tx_out.clone())
        }
    }

    struct ProcessOutputAllocatorCallbacks {
        template: WalletAbiWalletOutputTemplate,
    }

    impl WalletAbiOutputAllocatorCallbacks for ProcessOutputAllocatorCallbacks {
        fn get_wallet_output_template(
            &self,
            _session: WalletAbiRequestSession,
            _request: WalletAbiWalletOutputRequest,
        ) -> Result<WalletAbiWalletOutputTemplate, LwkError> {
            Ok(self.template.clone())
        }
    }

    #[test]
    fn wallet_abi_provider_get_raw_signing_x_only_pubkey() {
        let secp = Secp256k1::new();
        let secret_key = SecretKey::from_slice(&[0x11; 32]).expect("secret key");
        let keypair = Keypair::from_secret_key(&secp, &secret_key);
        let expected = keypair.x_only_public_key().0.to_string();
        let provider = WalletAbiProvider::new(
            Arc::new(SignerMetaLink::new(Arc::new(TestSignerCallbacks {
                keypair,
            }))),
            Arc::new(WalletRuntimeDepsLink::new(
                Arc::new(WalletSessionFactoryLink::new(Arc::new(
                    TestSessionFactoryCallbacks,
                ))),
                Arc::new(WalletOutputAllocatorLink::new(Arc::new(
                    TestOutputAllocatorCallbacks,
                ))),
                Arc::new(WalletPrevoutResolverLink::new(Arc::new(
                    TestPrevoutResolverCallbacks,
                ))),
                Arc::new(WalletBroadcasterLink::new(Arc::new(
                    TestBroadcasterCallbacks,
                ))),
                Arc::new(WalletReceiveAddressProviderLink::new(Arc::new(
                    TestReceiveAddressProviderCallbacks,
                ))),
            )),
        );

        assert_eq!(
            provider
                .get_raw_signing_x_only_pubkey()
                .expect("x-only public key")
                .to_string(),
            expected
        );
    }

    #[test]
    fn wallet_abi_provider_get_signer_receive_address() {
        let secp = Secp256k1::new();
        let secret_key = SecretKey::from_slice(&[0x22; 32]).expect("secret key");
        let keypair = Keypair::from_secret_key(&secp, &secret_key);
        let expected_address = "tlq1qq2xvpcvfup5j8zscjq05u2wxxjcyewk7979f3mmz5l7uw5pqmx6xf5xy50hsn6vhkm5euwt72x878eq6zxx2z58hd7zrsg9qn";
        let provider = WalletAbiProvider::new(
            Arc::new(SignerMetaLink::new(Arc::new(TestSignerCallbacks {
                keypair,
            }))),
            Arc::new(WalletRuntimeDepsLink::new(
                Arc::new(WalletSessionFactoryLink::new(Arc::new(
                    TestSessionFactoryCallbacks,
                ))),
                Arc::new(WalletOutputAllocatorLink::new(Arc::new(
                    TestOutputAllocatorCallbacks,
                ))),
                Arc::new(WalletPrevoutResolverLink::new(Arc::new(
                    TestPrevoutResolverCallbacks,
                ))),
                Arc::new(WalletBroadcasterLink::new(Arc::new(
                    TestBroadcasterCallbacks,
                ))),
                Arc::new(WalletReceiveAddressProviderLink::new(Arc::new(
                    TestReceiveAddressProviderCallbacks,
                ))),
            )),
        );

        assert_eq!(
            provider
                .get_signer_receive_address()
                .expect("receive address")
                .to_string(),
            expected_address
        );
    }

    #[test]
    fn wallet_abi_provider_process_request() {
        let network = Network::testnet();
        let mnemonic = Mnemonic::new(lwk_test_util::TEST_MNEMONIC).expect("mnemonic");
        let signer = Signer::new(&mnemonic, &network).expect("signer");
        let signer_link = SignerMetaLink::from_software_signer(
            signer.clone(),
            WalletAbiSignerContext {
                network: network.clone(),
                account_index: 0,
            },
        )
        .expect("signer link");
        let derivation_pair = wallet_abi_bip32_derivation_pair_from_signer(
            &signer,
            vec![84 + (1 << 31), 1 + (1 << 31), 1 << 31, 0, 0],
        )
        .expect("derivation pair");
        let wallet_pubkey =
            elements::bitcoin::PublicKey::from_str(&derivation_pair.pubkey).expect("pubkey");
        let blinding_pubkey = elements::secp256k1_zkp::PublicKey::from_str(
            "0279be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798",
        )
        .expect("blinding pubkey");
        let wallet_address = Arc::new(Address::from(elements::Address::p2wpkh(
            &wallet_pubkey,
            Some(blinding_pubkey),
            lwk_wollet::ElementsNetwork::LiquidTestnet.address_params(),
        )));
        let policy_asset = network.policy_asset();
        let outpoint = OutPoint::from_parts(
            &Txid::from_str("0000000000000000000000000000000000000000000000000000000000000001")
                .expect("txid"),
            0,
        );
        let tx_out = TxOut::from_explicit(
            wallet_address.script_pubkey().as_ref(),
            policy_asset,
            20_000,
        );
        let tx_out_secrets = TxOutSecrets::from_explicit(policy_asset, 20_000);
        let request = WalletAbiTxCreateRequest::from_parts(
            "0d6d53cd-a040-4f0c-8d28-c67b6608fb14",
            &network,
            &WalletAbiRuntimeParams::new(
                &[WalletAbiInputSchema::from_sequence(
                    "wallet-input",
                    &WalletAbiUtxoSource::wallet(&WalletAbiWalletSourceFilter::with_filters(
                        &WalletAbiAssetFilter::exact(policy_asset),
                        &WalletAbiAmountFilter::exact(20_000),
                        &WalletAbiLockFilter::none(),
                    )),
                    &WalletAbiInputUnblinding::wallet(),
                    &TxSequence::max(),
                    &WalletAbiFinalizerSpec::wallet(),
                )],
                &[crate::WalletAbiOutputSchema::new(
                    "external",
                    5_000,
                    &WalletAbiLockVariant::script(
                        &Address::new(
                            "tlq1qq02egjncr8g4qn890mrw3jhgupwqymekv383lwpmsfghn36hac5ptpmeewtnftluqyaraa56ung7wf47crkn5fjuhk422d68m",
                        )
                        .expect("external address")
                        .script_pubkey(),
                    ),
                    &WalletAbiAssetVariant::asset_id(policy_asset),
                    &WalletAbiBlinderVariant::explicit(),
                )],
                Some(0.0),
                None,
            ),
            false,
        )
        .expect("request");
        let provider = WalletAbiProvider::new(
            Arc::new(signer_link),
            Arc::new(WalletRuntimeDepsLink::new(
                Arc::new(WalletSessionFactoryLink::new(Arc::new(
                    ProcessSessionFactoryCallbacks {
                        session: WalletAbiRequestSession {
                            session_id: "session-1".to_string(),
                            network: network.clone(),
                            spendable_utxos: vec![ExternalUtxo::from_unchecked_data(
                                &outpoint,
                                &tx_out,
                                &tx_out_secrets,
                                107,
                            )],
                        },
                    },
                ))),
                Arc::new(WalletOutputAllocatorLink::new(Arc::new(
                    ProcessOutputAllocatorCallbacks {
                        template: crate::wallet_abi_output_template_from_address(&wallet_address),
                    },
                ))),
                Arc::new(WalletPrevoutResolverLink::new(Arc::new(
                    ProcessPrevoutResolverCallbacks {
                        derivation_pair,
                        tx_out: tx_out.clone(),
                        secrets: tx_out_secrets.clone(),
                    },
                ))),
                Arc::new(WalletBroadcasterLink::new(Arc::new(
                    TestBroadcasterCallbacks,
                ))),
                Arc::new(WalletReceiveAddressProviderLink::new(Arc::new(
                    TestReceiveAddressProviderCallbacks,
                ))),
            )),
        );

        let response = provider.process_request(&request).expect("process request");

        assert!(response.transaction().is_some());
        assert!(response.preview().expect("preview accessor").is_some());
        assert!(response.error_info().is_none());
    }

    #[test]
    fn wallet_abi_provider_get_capabilities() {
        let secp = Secp256k1::new();
        let secret_key = SecretKey::from_slice(&[0x33; 32]).expect("secret key");
        let keypair = Keypair::from_secret_key(&secp, &secret_key);
        let provider = WalletAbiProvider::new(
            Arc::new(SignerMetaLink::new(Arc::new(TestSignerCallbacks {
                keypair,
            }))),
            Arc::new(WalletRuntimeDepsLink::new(
                Arc::new(WalletSessionFactoryLink::new(Arc::new(
                    TestSessionFactoryCallbacks,
                ))),
                Arc::new(WalletOutputAllocatorLink::new(Arc::new(
                    TestOutputAllocatorCallbacks,
                ))),
                Arc::new(WalletPrevoutResolverLink::new(Arc::new(
                    TestPrevoutResolverCallbacks,
                ))),
                Arc::new(WalletBroadcasterLink::new(Arc::new(
                    TestBroadcasterCallbacks,
                ))),
                Arc::new(WalletReceiveAddressProviderLink::new(Arc::new(
                    TestReceiveAddressProviderCallbacks,
                ))),
            )),
        );

        let capabilities = provider.get_capabilities().expect("capabilities");

        assert_eq!(capabilities.network(), Network::testnet());
        assert_eq!(
            capabilities.methods(),
            vec![
                GET_RAW_SIGNING_X_ONLY_PUBKEY_METHOD.to_string(),
                GET_SIGNER_RECEIVE_ADDRESS_METHOD.to_string(),
                WALLET_ABI_EVALUATE_REQUEST_METHOD.to_string(),
                WALLET_ABI_GET_CAPABILITIES_METHOD.to_string(),
                WALLET_ABI_PROCESS_REQUEST_METHOD.to_string(),
            ]
        );
    }

    #[test]
    fn wallet_abi_provider_evaluate_request() {
        let network = Network::testnet();
        let mnemonic = Mnemonic::new(lwk_test_util::TEST_MNEMONIC).expect("mnemonic");
        let signer = Signer::new(&mnemonic, &network).expect("signer");
        let signer_link = SignerMetaLink::from_software_signer(
            signer.clone(),
            WalletAbiSignerContext {
                network: network.clone(),
                account_index: 0,
            },
        )
        .expect("signer link");
        let derivation_pair = wallet_abi_bip32_derivation_pair_from_signer(
            &signer,
            vec![84 + (1 << 31), 1 + (1 << 31), 1 << 31, 0, 0],
        )
        .expect("derivation pair");
        let wallet_pubkey =
            elements::bitcoin::PublicKey::from_str(&derivation_pair.pubkey).expect("pubkey");
        let blinding_pubkey = elements::secp256k1_zkp::PublicKey::from_str(
            "0279be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798",
        )
        .expect("blinding pubkey");
        let wallet_address = Arc::new(Address::from(elements::Address::p2wpkh(
            &wallet_pubkey,
            Some(blinding_pubkey),
            lwk_wollet::ElementsNetwork::LiquidTestnet.address_params(),
        )));
        let policy_asset = network.policy_asset();
        let outpoint = OutPoint::from_parts(
            &Txid::from_str("0000000000000000000000000000000000000000000000000000000000000001")
                .expect("txid"),
            0,
        );
        let tx_out = TxOut::from_explicit(
            wallet_address.script_pubkey().as_ref(),
            policy_asset,
            20_000,
        );
        let tx_out_secrets = TxOutSecrets::from_explicit(policy_asset, 20_000);
        let request = WalletAbiTxEvaluateRequest::from_parts(
            "0d6d53cd-a040-4f0c-8d28-c67b6608fb14",
            &network,
            &WalletAbiRuntimeParams::new(
                &[WalletAbiInputSchema::from_sequence(
                    "wallet-input",
                    &WalletAbiUtxoSource::wallet(&WalletAbiWalletSourceFilter::with_filters(
                        &WalletAbiAssetFilter::exact(policy_asset),
                        &WalletAbiAmountFilter::exact(20_000),
                        &WalletAbiLockFilter::none(),
                    )),
                    &WalletAbiInputUnblinding::wallet(),
                    &TxSequence::max(),
                    &WalletAbiFinalizerSpec::wallet(),
                )],
                &[crate::WalletAbiOutputSchema::new(
                    "external",
                    5_000,
                    &WalletAbiLockVariant::script(
                        &Address::new(
                            "tlq1qq02egjncr8g4qn890mrw3jhgupwqymekv383lwpmsfghn36hac5ptpmeewtnftluqyaraa56ung7wf47crkn5fjuhk422d68m",
                        )
                        .expect("external address")
                        .script_pubkey(),
                    ),
                    &WalletAbiAssetVariant::asset_id(policy_asset),
                    &WalletAbiBlinderVariant::explicit(),
                )],
                Some(0.0),
                None,
            ),
        )
        .expect("request");
        let provider = WalletAbiProvider::new(
            Arc::new(signer_link),
            Arc::new(WalletRuntimeDepsLink::new(
                Arc::new(WalletSessionFactoryLink::new(Arc::new(
                    ProcessSessionFactoryCallbacks {
                        session: WalletAbiRequestSession {
                            session_id: "session-1".to_string(),
                            network: network.clone(),
                            spendable_utxos: vec![ExternalUtxo::from_unchecked_data(
                                &outpoint,
                                &tx_out,
                                &tx_out_secrets,
                                107,
                            )],
                        },
                    },
                ))),
                Arc::new(WalletOutputAllocatorLink::new(Arc::new(
                    ProcessOutputAllocatorCallbacks {
                        template: crate::wallet_abi_output_template_from_address(&wallet_address),
                    },
                ))),
                Arc::new(WalletPrevoutResolverLink::new(Arc::new(
                    ProcessPrevoutResolverCallbacks {
                        derivation_pair,
                        tx_out: tx_out.clone(),
                        secrets: tx_out_secrets.clone(),
                    },
                ))),
                Arc::new(WalletBroadcasterLink::new(Arc::new(
                    TestBroadcasterCallbacks,
                ))),
                Arc::new(WalletReceiveAddressProviderLink::new(Arc::new(
                    TestReceiveAddressProviderCallbacks,
                ))),
            )),
        );

        let response = provider
            .evaluate_request(&request)
            .expect("evaluate request");

        assert!(response.preview().is_some());
        assert!(response.error_info().is_none());
    }

    #[test]
    fn wallet_abi_provider_dispatch_json_matches_address_getter() {
        let secp = Secp256k1::new();
        let secret_key = SecretKey::from_slice(&[0x44; 32]).expect("secret key");
        let keypair = Keypair::from_secret_key(&secp, &secret_key);
        let provider = WalletAbiProvider::new(
            Arc::new(SignerMetaLink::new(Arc::new(TestSignerCallbacks {
                keypair,
            }))),
            Arc::new(WalletRuntimeDepsLink::new(
                Arc::new(WalletSessionFactoryLink::new(Arc::new(
                    TestSessionFactoryCallbacks,
                ))),
                Arc::new(WalletOutputAllocatorLink::new(Arc::new(
                    TestOutputAllocatorCallbacks,
                ))),
                Arc::new(WalletPrevoutResolverLink::new(Arc::new(
                    TestPrevoutResolverCallbacks,
                ))),
                Arc::new(WalletBroadcasterLink::new(Arc::new(
                    TestBroadcasterCallbacks,
                ))),
                Arc::new(WalletReceiveAddressProviderLink::new(Arc::new(
                    TestReceiveAddressProviderCallbacks,
                ))),
            )),
        );

        let typed_json = serde_json::to_string(
            &provider
                .get_signer_receive_address()
                .expect("receive address")
                .to_string(),
        )
        .expect("typed json");
        let dispatch_json = provider
            .dispatch_json(GET_SIGNER_RECEIVE_ADDRESS_METHOD, "null")
            .expect("dispatch json");

        assert_eq!(dispatch_json, typed_json);
    }

    #[test]
    fn wallet_abi_provider_dispatch_json_rejects_unknown_method() {
        let secp = Secp256k1::new();
        let secret_key = SecretKey::from_slice(&[0x55; 32]).expect("secret key");
        let keypair = Keypair::from_secret_key(&secp, &secret_key);
        let provider = WalletAbiProvider::new(
            Arc::new(SignerMetaLink::new(Arc::new(TestSignerCallbacks {
                keypair,
            }))),
            Arc::new(WalletRuntimeDepsLink::new(
                Arc::new(WalletSessionFactoryLink::new(Arc::new(
                    TestSessionFactoryCallbacks,
                ))),
                Arc::new(WalletOutputAllocatorLink::new(Arc::new(
                    TestOutputAllocatorCallbacks,
                ))),
                Arc::new(WalletPrevoutResolverLink::new(Arc::new(
                    TestPrevoutResolverCallbacks,
                ))),
                Arc::new(WalletBroadcasterLink::new(Arc::new(
                    TestBroadcasterCallbacks,
                ))),
                Arc::new(WalletReceiveAddressProviderLink::new(Arc::new(
                    TestReceiveAddressProviderCallbacks,
                ))),
            )),
        );

        assert_eq!(
            provider
                .dispatch_json("wallet_abi_unknown", "null")
                .expect_err("unknown method")
                .to_string(),
            "unsupported wallet-abi method 'wallet_abi_unknown'".to_string()
        );
    }

    #[test]
    fn wallet_abi_provider_dispatch_json_rejects_getter_params() {
        let secp = Secp256k1::new();
        let secret_key = SecretKey::from_slice(&[0x66; 32]).expect("secret key");
        let keypair = Keypair::from_secret_key(&secp, &secret_key);
        let provider = WalletAbiProvider::new(
            Arc::new(SignerMetaLink::new(Arc::new(TestSignerCallbacks {
                keypair,
            }))),
            Arc::new(WalletRuntimeDepsLink::new(
                Arc::new(WalletSessionFactoryLink::new(Arc::new(
                    TestSessionFactoryCallbacks,
                ))),
                Arc::new(WalletOutputAllocatorLink::new(Arc::new(
                    TestOutputAllocatorCallbacks,
                ))),
                Arc::new(WalletPrevoutResolverLink::new(Arc::new(
                    TestPrevoutResolverCallbacks,
                ))),
                Arc::new(WalletBroadcasterLink::new(Arc::new(
                    TestBroadcasterCallbacks,
                ))),
                Arc::new(WalletReceiveAddressProviderLink::new(Arc::new(
                    TestReceiveAddressProviderCallbacks,
                ))),
            )),
        );

        assert_eq!(
            provider
                .dispatch_json(GET_SIGNER_RECEIVE_ADDRESS_METHOD, "{}")
                .expect_err("bad params")
                .to_string(),
            "wallet-abi method 'get_signer_receive_address' does not accept params".to_string()
        );
    }
}
