//! Wallet-ABI JSON-RPC provider.

mod json_rpc;

use crate::error::WalletAbiError;
use crate::wallet_abi::schema::{SignerMeta, TxCreateRequest, TxCreateResponse, WalletMeta};
use crate::wallet_abi::tx_resolution::runtime::Runtime;

use self::json_rpc::{JsonRpcErrorCode, JsonRpcRequest, JsonRpcResponse, JsonRpcResultPayload};
pub use self::json_rpc::{
    RawSigningXOnlyPubkeyResult, SignerReceiveAddressResult, GET_RAW_SIGNING_X_ONLY_PUBKEY_METHOD,
    GET_SIGNER_RECEIVE_ADDRESS_METHOD, WALLET_ABI_PROCESS_REQUEST_METHOD,
};

/// Long-lived wallet-abi JSON-RPC provider.
pub struct WalletAbiProvider<Signer, Wallet> {
    signer: Signer,
    wallet: Wallet,
}

impl<Signer, Wallet> WalletAbiProvider<Signer, Wallet>
where
    Signer: SignerMeta,
    Wallet: WalletMeta,
    WalletAbiError: From<Signer::Error> + From<Wallet::Error>,
{
    /// Create a provider from runtime signer and wallet dependencies.
    pub fn new(signer: Signer, wallet: Wallet) -> Self {
        Self { signer, wallet }
    }

    /// Process one WalletKit-style JSON-RPC request.
    ///
    /// Outer JSON/envelope parsing failures return `Err`. Parsed requests always
    /// serialize into a JSON-RPC result or error response.
    pub fn process_json_rpc_request(&self, request_json: &str) -> Result<String, WalletAbiError> {
        let request: JsonRpcRequest = serde_json::from_str(request_json)?;
        let response = match request {
            JsonRpcRequest::GetSignerReceiveAddress { id } => {
                self.handle_get_signer_receive_address(id)
            }
            JsonRpcRequest::GetRawSigningXOnlyPubkey { id } => {
                self.handle_get_raw_signing_x_only_pubkey(id)
            }
            JsonRpcRequest::WalletAbiProcessRequest {
                id,
                tx_create_request,
            } => self.handle_process_request(id, tx_create_request),
            JsonRpcRequest::Rejected { response } => response,
        };

        serde_json::to_string(&response).map_err(Into::into)
    }

    fn handle_get_signer_receive_address(&self, id: i64) -> JsonRpcResponse {
        let signer_receive_address = match self.signer.get_signer_receive_address() {
            Ok(address) => address.to_string(),
            Err(error) => {
                return JsonRpcResponse::error(
                    id,
                    JsonRpcErrorCode::InternalError,
                    format!(
                        "failed to read signer receive address: {}",
                        WalletAbiError::from(error)
                    ),
                );
            }
        };

        JsonRpcResponse::result(
            id,
            JsonRpcResultPayload::SignerReceiveAddress(json_rpc::SignerReceiveAddressResult {
                signer_receive_address,
            }),
        )
    }

    fn handle_get_raw_signing_x_only_pubkey(&self, id: i64) -> JsonRpcResponse {
        let signing_x_only_pubkey = match self.signer.get_raw_signing_x_only_pubkey() {
            Ok(pubkey) => pubkey.to_string(),
            Err(error) => {
                return JsonRpcResponse::error(
                    id,
                    JsonRpcErrorCode::InternalError,
                    format!(
                        "failed to read signer x-only public key: {}",
                        WalletAbiError::from(error)
                    ),
                );
            }
        };

        JsonRpcResponse::result(
            id,
            JsonRpcResultPayload::RawSigningXOnlyPubkey(json_rpc::RawSigningXOnlyPubkeyResult {
                raw_signing_x_only_pubkey: signing_x_only_pubkey,
            }),
        )
    }

    fn handle_process_request(
        &self,
        id: i64,
        tx_create_request: TxCreateRequest,
    ) -> JsonRpcResponse {
        let runtime = match tokio::runtime::Builder::new_current_thread().build() {
            Ok(runtime) => runtime,
            Err(error) => {
                return JsonRpcResponse::error(
                    id,
                    JsonRpcErrorCode::InternalError,
                    format!("wallet-abi runtime initialization failed: {error}"),
                );
            }
        };

        let request_for_runtime = tx_create_request.clone();
        let runtime_result = runtime.block_on(async {
            Runtime::build(request_for_runtime, &self.signer, &self.wallet)
                .process_request()
                .await
        });

        // Wallet-ABI runtime failures stay inside the method-specific envelope so
        // callers always get a JSON-RPC success carrying a serialized ABI response.
        let response = match runtime_result {
            Ok(response) => response,
            Err(error) => TxCreateResponse::error(&tx_create_request, &error),
        };

        match serde_json::to_value(&response) {
            Ok(_) => JsonRpcResponse::result(id, JsonRpcResultPayload::TxCreateResponse(response)),
            Err(error) => JsonRpcResponse::error(
                id,
                JsonRpcErrorCode::InternalError,
                format!("wallet-abi response json serialization failed: {error}"),
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;
    use std::{env, fs, path::PathBuf};

    use super::json_rpc::{
        JsonRpcResponse, JsonRpcResult, JsonRpcResultPayload, JsonRpcVersion,
        RawSigningXOnlyPubkeyResult, SignerReceiveAddressResult,
        GET_RAW_SIGNING_X_ONLY_PUBKEY_METHOD, GET_SIGNER_RECEIVE_ADDRESS_METHOD,
    };
    use super::WalletAbiProvider;
    use crate::error::WalletAbiError;
    use crate::wallet_abi::schema::{
        AssetVariant, BlinderVariant, ErrorInfo, FinalizerSpec, InputSchema, LockVariant,
        OutputSchema, RuntimeParams, SignerMeta, TxCreateRequest, TxCreateResponse, UTXOSource,
        WalletMeta, WalletSourceFilter, TX_CREATE_ABI_VERSION,
    };

    use lwk_common::{Bip, Network};

    use lwk_wollet::bitcoin::bip32::{DerivationPath, Fingerprint};
    use lwk_wollet::bitcoin::PublicKey;
    use lwk_wollet::elements::pset::PartiallySignedTransaction;
    use lwk_wollet::elements::{Address, OutPoint, Transaction, TxOut, TxOutSecrets, Txid};
    use lwk_wollet::secp256k1::schnorr::Signature;
    use lwk_wollet::secp256k1::{Message, XOnlyPublicKey};
    use lwk_wollet::WalletTxOut;

    struct TestSigner;
    struct ErrorSigner;
    struct EmptyWallet;

    impl SignerMeta for TestSigner {
        type Error = WalletAbiError;

        fn get_network(&self) -> Network {
            Network::LocaltestLiquid
        }

        fn get_signer_receive_address(&self) -> Result<Address, Self::Error> {
            Address::from_str(
                "tlq1qq2xvpcvfup5j8zscjq05u2wxxjcyewk7979f3mmz5l7uw5pqmx6xf5xy50hsn6vhkm5euwt72x878eq6zxx2z58hd7zrsg9qn",
            )
            .map_err(|error| WalletAbiError::InvalidSignerConfig(error.to_string()))
        }

        fn fingerprint(&self) -> Fingerprint {
            Fingerprint::from([1, 2, 3, 4])
        }

        fn get_derivation_path(&self, _bip: Bip) -> DerivationPath {
            DerivationPath::default()
        }

        fn get_pubkey_by_derivation_path(
            &self,
            _derivation_path: &DerivationPath,
        ) -> Result<PublicKey, Self::Error> {
            PublicKey::from_str(
                "0279be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798",
            )
            .map_err(|error| WalletAbiError::InvalidSignerConfig(error.to_string()))
        }

        fn get_raw_signing_x_only_pubkey(&self) -> Result<XOnlyPublicKey, Self::Error> {
            XOnlyPublicKey::from_str(
                "79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798",
            )
            .map_err(|error| WalletAbiError::InvalidSignerConfig(error.to_string()))
        }

        fn unblind(&self, _tx_out: &TxOut) -> Result<TxOutSecrets, Self::Error> {
            unreachable!("not used in provider tests")
        }

        fn sign_pst(&self, _pst: &mut PartiallySignedTransaction) -> Result<(), Self::Error> {
            unreachable!("not used in provider tests")
        }

        fn sign_schnorr(
            &self,
            _message: Message,
            _xonly_public_key: XOnlyPublicKey,
        ) -> Result<Signature, Self::Error> {
            unreachable!("not used in provider tests")
        }
    }

    impl SignerMeta for ErrorSigner {
        type Error = WalletAbiError;

        fn get_network(&self) -> Network {
            Network::LocaltestLiquid
        }

        fn get_signer_receive_address(&self) -> Result<Address, Self::Error> {
            Err(WalletAbiError::InvalidSignerConfig(
                "missing receive address".to_string(),
            ))
        }

        fn fingerprint(&self) -> Fingerprint {
            Fingerprint::from([1, 2, 3, 4])
        }

        fn get_derivation_path(&self, _bip: Bip) -> DerivationPath {
            DerivationPath::default()
        }

        fn get_pubkey_by_derivation_path(
            &self,
            _derivation_path: &DerivationPath,
        ) -> Result<PublicKey, Self::Error> {
            PublicKey::from_str(
                "0279be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798",
            )
            .map_err(|error| WalletAbiError::InvalidSignerConfig(error.to_string()))
        }

        fn get_raw_signing_x_only_pubkey(&self) -> Result<XOnlyPublicKey, Self::Error> {
            Err(WalletAbiError::InvalidSignerConfig(
                "missing signing pubkey".to_string(),
            ))
        }

        fn unblind(&self, _tx_out: &TxOut) -> Result<TxOutSecrets, Self::Error> {
            unreachable!("not used in provider tests")
        }

        fn sign_pst(&self, _pst: &mut PartiallySignedTransaction) -> Result<(), Self::Error> {
            unreachable!("not used in provider tests")
        }

        fn sign_schnorr(
            &self,
            _message: Message,
            _xonly_public_key: XOnlyPublicKey,
        ) -> Result<Signature, Self::Error> {
            unreachable!("not used in provider tests")
        }
    }

    impl WalletMeta for EmptyWallet {
        type Error = WalletAbiError;

        fn get_tx_out(
            &self,
            _outpoint: OutPoint,
        ) -> impl std::future::Future<Output = Result<TxOut, Self::Error>> + Send + '_ {
            async {
                Err(WalletAbiError::InvalidResponse(
                    "not used for this test".to_string(),
                ))
            }
        }

        fn broadcast_transaction(
            &self,
            _tx: Transaction,
        ) -> impl std::future::Future<Output = Result<Txid, Self::Error>> + Send + '_ {
            async {
                Err(WalletAbiError::InvalidResponse(
                    "not used for this test".to_string(),
                ))
            }
        }

        fn get_spendable_utxos(
            &self,
        ) -> impl std::future::Future<Output = Result<Vec<WalletTxOut>, Self::Error>> + Send + '_
        {
            async { Ok(Vec::new()) }
        }
    }

    #[test]
    fn process_json_rpc_request_returns_signer_receive_address() {
        let provider = WalletAbiProvider::new(TestSigner, EmptyWallet);
        let request = json_rpc_request(2, GET_SIGNER_RECEIVE_ADDRESS_METHOD, None);

        let response = provider
            .process_json_rpc_request(&request)
            .expect("json-rpc response");
        let response: JsonRpcResponse = serde_json::from_str(&response).expect("parse response");

        assert_eq!(
            response,
            JsonRpcResponse::Result(JsonRpcResult {
                id: 2,
                jsonrpc: JsonRpcVersion::V2,
                result: JsonRpcResultPayload::SignerReceiveAddress(SignerReceiveAddressResult {
                    signer_receive_address: "tlq1qq2xvpcvfup5j8zscjq05u2wxxjcyewk7979f3mmz5l7uw5pqmx6xf5xy50hsn6vhkm5euwt72x878eq6zxx2z58hd7zrsg9qn".to_string(),
                }),
            })
        );
    }

    #[test]
    fn process_json_rpc_request_returns_raw_signing_x_only_pubkey() {
        let provider = WalletAbiProvider::new(TestSigner, EmptyWallet);
        let request = json_rpc_request(3, GET_RAW_SIGNING_X_ONLY_PUBKEY_METHOD, None);

        let response = provider
            .process_json_rpc_request(&request)
            .expect("json-rpc response");
        let response: JsonRpcResponse = serde_json::from_str(&response).expect("parse response");

        assert_eq!(
            response,
            JsonRpcResponse::Result(JsonRpcResult {
                id: 3,
                jsonrpc: JsonRpcVersion::V2,
                result: JsonRpcResultPayload::RawSigningXOnlyPubkey(RawSigningXOnlyPubkeyResult {
                    raw_signing_x_only_pubkey:
                        "79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798"
                            .to_string(),
                },),
            })
        );
    }

    #[test]
    fn process_json_rpc_request_returns_tx_create_response() {
        let provider = WalletAbiProvider::new(TestSigner, EmptyWallet);
        let request = json_rpc_request(
            4,
            super::WALLET_ABI_PROCESS_REQUEST_METHOD,
            Some(serde_json::to_value(sample_tx_create_request()).expect("request json")),
        );

        let response = provider
            .process_json_rpc_request(&request)
            .expect("json-rpc response");
        let response: JsonRpcResponse = serde_json::from_str(&response).expect("parse response");

        match response {
            JsonRpcResponse::Result(JsonRpcResult {
                id,
                jsonrpc,
                result: JsonRpcResultPayload::TxCreateResponse(result),
            }) => {
                assert_eq!(id, 4);
                assert_eq!(jsonrpc, JsonRpcVersion::V2);
                assert_eq!(
                    result.status,
                    crate::wallet_abi::schema::tx_create::Status::Error
                );
                assert_eq!(result.request_id, sample_tx_create_request().request_id);
            }
            other => panic!("unexpected response: {other:?}"),
        }
    }

    #[test]
    fn wallet_abi_contract_fixtures_match_sdk_repo() {
        let Some(fixtures_dir) = sdk_contract_fixtures_dir() else {
            return;
        };

        assert_fixture_eq(
            fixtures_dir.join("error_info.json"),
            serde_json::to_value(ErrorInfo {
                code: crate::wallet_abi::schema::WalletAbiErrorCode::InvalidRequest,
                message: "request abi_version mismatch".to_string(),
                details: Some(serde_json::json!({
                    "field": "abi_version",
                    "expected": TX_CREATE_ABI_VERSION,
                    "actual": "wallet-abi-9.9",
                })),
            })
            .expect("serialize error info"),
        );

        assert_fixture_eq(
            fixtures_dir.join("tx_create_request.json"),
            serde_json::to_value(sample_tx_create_request()).expect("serialize tx create request"),
        );

        assert_fixture_eq(
            fixtures_dir.join("tx_create_response.json"),
            serde_json::to_value(sample_tx_create_response())
                .expect("serialize tx create response"),
        );

        let internal_key_source_external = serde_json::json!({
            "external": {
                "key": {
                    "identity": {
                        "ExternalXOnly": "79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798"
                    },
                    "pubkey": "0279be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798",
                    "address": "tlq1qq2xvpcvfup5j8zscjq05u2wxxjcyewk7979f3mmz5l7uw5pqmx6xf5xy50hsn6vhkm5euwt72x878eq6zxx2z58hd7zrsg9qn"
                }
            }
        });
        assert_fixture_eq(
            fixtures_dir.join("internal_key_source_external.json"),
            internal_key_source_external.clone(),
        );

        assert_fixture_eq(
            fixtures_dir.join("simf_finalizer.json"),
            serde_json::json!({
                "type": "simf",
                "source_simf": "main := witness::sig_all;",
                "internal_key": internal_key_source_external,
                "arguments": [123, 34, 114, 101, 115, 111, 108, 118, 101, 100, 34, 58, 123, 125, 44, 34, 114, 117, 110, 116, 105, 109, 101, 95, 97, 114, 103, 117, 109, 101, 110, 116, 115, 34, 58, 123, 125, 125],
                "witness": [123, 34, 114, 101, 115, 111, 108, 118, 101, 100, 34, 58, 123, 125, 44, 34, 114, 117, 110, 116, 105, 109, 101, 95, 97, 114, 103, 117, 109, 101, 110, 116, 115, 34, 58, 91, 93, 125]
            }),
        );

        assert_fixture_eq(
            fixtures_dir.join("json_rpc_get_signer_receive_address_request.json"),
            serde_json::json!({
                "id": 1,
                "jsonrpc": "2.0",
                "method": GET_SIGNER_RECEIVE_ADDRESS_METHOD,
            }),
        );

        assert_fixture_eq(
            fixtures_dir.join("json_rpc_get_signer_receive_address_response.json"),
            serde_json::to_value(JsonRpcResponse::Result(JsonRpcResult {
                id: 1,
                jsonrpc: JsonRpcVersion::V2,
                result: JsonRpcResultPayload::SignerReceiveAddress(SignerReceiveAddressResult {
                    signer_receive_address: "tlq1qq2xvpcvfup5j8zscjq05u2wxxjcyewk7979f3mmz5l7uw5pqmx6xf5xy50hsn6vhkm5euwt72x878eq6zxx2z58hd7zrsg9qn".to_string(),
                }),
            }))
            .expect("serialize signer receive address response"),
        );

        assert_fixture_eq(
            fixtures_dir.join("json_rpc_get_raw_signing_x_only_pubkey_request.json"),
            serde_json::json!({
                "id": 2,
                "jsonrpc": "2.0",
                "method": GET_RAW_SIGNING_X_ONLY_PUBKEY_METHOD,
            }),
        );

        assert_fixture_eq(
            fixtures_dir.join("json_rpc_get_raw_signing_x_only_pubkey_response.json"),
            serde_json::to_value(JsonRpcResponse::Result(JsonRpcResult {
                id: 2,
                jsonrpc: JsonRpcVersion::V2,
                result: JsonRpcResultPayload::RawSigningXOnlyPubkey(RawSigningXOnlyPubkeyResult {
                    raw_signing_x_only_pubkey:
                        "79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798"
                            .to_string(),
                }),
            }))
            .expect("serialize raw signing x-only pubkey response"),
        );

        assert_fixture_eq(
            fixtures_dir.join("json_rpc_process_request.json"),
            serde_json::json!({
                "id": 3,
                "jsonrpc": "2.0",
                "method": "wallet_abi_process_request",
                "params": sample_tx_create_request(),
            }),
        );

        assert_fixture_eq(
            fixtures_dir.join("json_rpc_process_response.json"),
            serde_json::to_value(JsonRpcResponse::Result(JsonRpcResult {
                id: 3,
                jsonrpc: JsonRpcVersion::V2,
                result: JsonRpcResultPayload::TxCreateResponse(sample_tx_create_response()),
            }))
            .expect("serialize process response"),
        );
    }

    fn json_rpc_request(id: i64, method: &str, params: Option<serde_json::Value>) -> String {
        let mut request = serde_json::json!({
            "id": id,
            "jsonrpc": "2.0",
            "method": method,
        });

        if let Some(params) = params {
            request["params"] = params;
        }

        request.to_string()
    }

    fn sample_tx_create_request() -> TxCreateRequest {
        TxCreateRequest {
            abi_version: TX_CREATE_ABI_VERSION.to_string(),
            request_id: sample_request_id(),
            network: Network::LocaltestLiquid,
            params: RuntimeParams {
                inputs: vec![InputSchema {
                    id: "wallet-input-0".to_string(),
                    utxo_source: UTXOSource::Wallet {
                        filter: WalletSourceFilter::default(),
                    },
                    unblinding: crate::wallet_abi::schema::InputUnblinding::Wallet,
                    sequence: lwk_wollet::elements::Sequence::MAX,
                    issuance: None,
                    finalizer: FinalizerSpec::Wallet,
                }],
                outputs: vec![OutputSchema {
                    id: "recipient-0".to_string(),
                    amount_sat: 1_250,
                    lock: LockVariant::Script {
                        script: lwk_wollet::elements::Script::new(),
                    },
                    asset: AssetVariant::AssetId {
                        asset_id: *Network::LocaltestLiquid.policy_asset(),
                    },
                    blinder: BlinderVariant::Explicit,
                }],
                fee_rate_sat_kvb: Some(100.0),
                lock_time: Some(lwk_wollet::elements::LockTime::ZERO),
            },
            broadcast: false,
        }
    }

    fn sample_tx_create_response() -> TxCreateResponse {
        TxCreateResponse {
            abi_version: TX_CREATE_ABI_VERSION.to_string(),
            request_id: sample_request_id(),
            network: Network::LocaltestLiquid,
            status: crate::wallet_abi::schema::tx_create::Status::Error,
            transaction: None,
            artifacts: Some(
                [(
                    "transport".to_string(),
                    serde_json::Value::String("mock".to_string()),
                )]
                .into_iter()
                .collect(),
            ),
            error: Some(ErrorInfo {
                code: crate::wallet_abi::schema::WalletAbiErrorCode::Funding,
                message: "insufficient funds".to_string(),
                details: Some(serde_json::json!({
                    "asset_id": Network::LocaltestLiquid.policy_asset().to_string(),
                    "missing_sat": 1250,
                })),
            }),
        }
    }

    fn sample_request_id() -> uuid::Uuid {
        uuid::Uuid::parse_str("3d6f0a38-06c0-4a93-9dd8-72738d694a11").expect("valid uuid")
    }

    fn sdk_contract_fixtures_dir() -> Option<PathBuf> {
        let fixtures_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../wallet-abi-sdk/fixtures/contract");

        let expected_fixture =
            fixtures_dir.join("json_rpc_get_signer_receive_address_request.json");
        (fixtures_dir.exists() && expected_fixture.exists()).then_some(fixtures_dir)
    }

    fn assert_fixture_eq(path: PathBuf, actual: serde_json::Value) {
        let fixture = fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("failed to read fixture {}: {error}", path.display()));
        let expected: serde_json::Value =
            serde_json::from_str(&fixture).expect("parse fixture json");
        assert_eq!(actual, expected, "fixture mismatch for {}", path.display());
    }
}
