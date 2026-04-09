use std::future::Future;
use std::sync::Arc;

use crate::{
    Address, LwkError, SignerMetaLink, WalletAbiCapabilities, WalletAbiTxCreateRequest,
    WalletAbiTxCreateResponse, WalletAbiTxEvaluateRequest, WalletAbiTxEvaluateResponse,
    WalletBroadcasterLink, WalletOutputAllocatorLink, WalletPrevoutResolverLink,
    WalletRuntimeDepsLink, WalletSessionFactoryLink, XOnlyPublicKey,
};
use lwk_simplicity::wallet_abi::{
    GET_RAW_SIGNING_X_ONLY_PUBKEY_METHOD, GET_SIGNER_RECEIVE_ADDRESS_METHOD, KeyStoreMeta,
    TxCreateRequest as SimplicityTxCreateRequest, TxEvaluateRequest as SimplicityTxEvaluateRequest,
    WalletAbiRuntime as SimplicityWalletAbiRuntime,
    WALLET_ABI_EVALUATE_REQUEST_METHOD, WALLET_ABI_GET_CAPABILITIES_METHOD,
    WALLET_ABI_PROCESS_REQUEST_METHOD,
    WalletBroadcaster as SimplicityWalletBroadcaster, WalletOutputAllocator, WalletOutputRequest,
    WalletOutputTemplate, WalletPrevoutResolver, WalletProviderMeta, WalletReceiveAddressProvider,
    WalletRequestSession as SimplicityWalletRequestSession,
    WalletRuntimeDeps as SimplicityWalletRuntimeDeps,
    WalletSessionFactory as SimplicityWalletSessionFactory,
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

    fn open_wallet_request_session(
        &self,
    ) -> impl Future<Output = Result<SimplicityWalletRequestSession, Self::Error>> + Send + '_ {
        async move {
            self.inner
                .open_wallet_request_session()
                .await
                .map_err(|error| ProviderRuntimeError(error.to_string()))
        }
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

    fn get_tx_out(
        &self,
        outpoint: ElementsOutPoint,
    ) -> impl Future<Output = Result<ElementsTxOut, Self::Error>> + Send + '_ {
        async move {
            self.prevout_resolver
                .get_tx_out(outpoint)
                .await
                .map_err(|error| ProviderRuntimeError(error.to_string()))
        }
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
        WalletAbiWalletOutputRequest, WalletAbiWalletOutputTemplate,
        WalletAbiWalletSourceFilter, WalletBroadcasterLink,
        WalletOutputAllocatorLink, WalletPrevoutResolverLink, WalletReceiveAddressProviderLink,
        WalletSessionFactoryLink,
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
            vec![84 + (1 << 31), 1 + (1 << 31), 0 + (1 << 31), 0, 0],
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
            vec![84 + (1 << 31), 1 + (1 << 31), 0 + (1 << 31), 0, 0],
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

        let response = provider.evaluate_request(&request).expect("evaluate request");

        assert!(response.preview().is_some());
        assert!(response.error_info().is_none());
    }
}
