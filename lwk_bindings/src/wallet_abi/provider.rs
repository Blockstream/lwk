use std::sync::Arc;

use lwk_simplicity::wallet_abi::WalletAbiProvider as CoreWalletAbiProvider;

use super::{SignerMetaLink, WalletMetaLink};
use crate::LwkError;

/// UniFFI bridge for Wallet-ABI JSON-RPC request processing.
#[derive(uniffi::Object)]
pub struct WalletAbiProvider {
    inner: CoreWalletAbiProvider<Arc<SignerMetaLink>, Arc<WalletMetaLink>>,
}

#[uniffi::export]
impl WalletAbiProvider {
    /// Create a provider object from foreign signer and wallet callback bridges.
    #[uniffi::constructor]
    pub fn new(signer: Arc<SignerMetaLink>, wallet: Arc<WalletMetaLink>) -> Self {
        Self {
            inner: CoreWalletAbiProvider::new(signer, wallet),
        }
    }

    /// Process one WalletKit-style JSON-RPC request string.
    pub fn process_json_rpc_request(&self, request_json: String) -> Result<String, LwkError> {
        self.inner
            .process_json_rpc_request(&request_json)
            .map_err(LwkError::from)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use lwk_simplicity::wallet_abi::provider::{
        RawSigningXOnlyPubkeyResult, SignerReceiveAddressResult,
        GET_RAW_SIGNING_X_ONLY_PUBKEY_METHOD, GET_SIGNER_RECEIVE_ADDRESS_METHOD,
        WALLET_ABI_PROCESS_REQUEST_METHOD,
    };
    use lwk_simplicity::wallet_abi::schema::{
        generate_request_id, tx_create::Status, RuntimeParams, TxCreateRequest, TxCreateResponse,
        TX_CREATE_ABI_VERSION,
    };
    use serde::Deserialize;
    use serde_json::Value;

    use super::{CoreWalletAbiProvider, WalletAbiProvider};
    use crate::types::{PublicKey, XOnlyPublicKey};
    use crate::wallet_abi::{
        SignerMetaLink, WalletAbiBip, WalletAbiSignerCallbacks, WalletAbiWalletCallbacks,
        WalletMetaLink,
    };
    use crate::{
        Address, LwkError, Network, OutPoint, Pset, Transaction, TxOut,
        TxOutSecrets as BindingsTxOutSecrets, Txid, WalletTxOut,
    };

    struct TestSigner;
    struct EmptyWallet;

    impl WalletAbiSignerCallbacks for TestSigner {
        fn get_network(&self) -> Arc<Network> {
            Network::regtest_default()
        }

        fn get_signer_receive_address(&self) -> Result<Arc<Address>, LwkError> {
            Address::new(
                "tlq1qq2xvpcvfup5j8zscjq05u2wxxjcyewk7979f3mmz5l7uw5pqmx6xf5xy50hsn6vhkm5euwt72x878eq6zxx2z58hd7zrsg9qn",
            )
        }

        fn fingerprint(&self) -> u32 {
            0x0102_0304
        }

        fn get_derivation_path(&self, bip: WalletAbiBip) -> Vec<u32> {
            let purpose = match bip {
                WalletAbiBip::Bip49 => 49,
                WalletAbiBip::Bip84 => 84,
                WalletAbiBip::Bip87 => 87,
            };
            vec![purpose | 0x8000_0000, 1 | 0x8000_0000, 0x8000_0000]
        }

        fn get_pubkey_by_derivation_path(
            &self,
            _derivation_path: Vec<u32>,
        ) -> Result<Arc<PublicKey>, LwkError> {
            PublicKey::from_string(
                "0279be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798",
            )
        }

        fn get_raw_signing_x_only_pubkey(&self) -> Result<Arc<XOnlyPublicKey>, LwkError> {
            XOnlyPublicKey::from_string(
                "79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798",
            )
        }

        fn unblind(&self, tx_out: Arc<TxOut>) -> Result<Arc<BindingsTxOutSecrets>, LwkError> {
            let asset = tx_out.asset().ok_or_else(|| LwkError::Generic {
                msg: "tx_out asset must be explicit for this test".to_string(),
            })?;
            Ok(BindingsTxOutSecrets::from_explicit(
                asset,
                tx_out.value().unwrap_or(0),
            ))
        }

        fn sign_pst(&self, pst: Arc<Pset>) -> Result<Arc<Pset>, LwkError> {
            Ok(pst)
        }

        fn sign_schnorr(&self, _message: Vec<u8>) -> Result<Vec<u8>, LwkError> {
            Ok(vec![1u8; 64])
        }
    }

    impl WalletAbiWalletCallbacks for EmptyWallet {
        fn get_tx_out(&self, _outpoint: Arc<OutPoint>) -> Result<Arc<TxOut>, LwkError> {
            Err(LwkError::Generic {
                msg: "not used for this test".to_string(),
            })
        }

        fn broadcast_transaction(&self, _tx: Arc<Transaction>) -> Result<Arc<Txid>, LwkError> {
            Err(LwkError::Generic {
                msg: "not used for this test".to_string(),
            })
        }

        fn get_spendable_utxos(&self) -> Result<Vec<Arc<WalletTxOut>>, LwkError> {
            Ok(Vec::new())
        }
    }

    #[derive(Debug, Deserialize)]
    #[serde(untagged)]
    enum JsonRpcResponseEnvelope {
        Result {
            id: i64,
            jsonrpc: String,
            result: Value,
        },
        Error {
            id: i64,
            jsonrpc: String,
            error: JsonRpcErrorObject,
        },
    }

    #[derive(Debug, Deserialize)]
    struct JsonRpcErrorObject {
        code: i32,
        message: String,
    }

    #[test]
    fn wallet_abi_provider_rejects_malformed_outer_json() {
        let provider = make_wallet_abi_provider();

        let error = provider
            .process_json_rpc_request("{bad-json".to_string())
            .expect_err("malformed outer json must fail");
        match error {
            LwkError::Generic { msg } => assert!(msg.contains("Serde")),
            other => panic!("unexpected error variant: {other:?}"),
        }
    }

    #[test]
    fn wallet_abi_provider_returns_signer_receive_address_response() {
        let provider = make_wallet_abi_provider();

        let response_json = provider
            .process_json_rpc_request(json_rpc_request(1, GET_SIGNER_RECEIVE_ADDRESS_METHOD, None))
            .expect("must return json");
        let response: JsonRpcResponseEnvelope =
            serde_json::from_str(&response_json).expect("response parse");

        match response {
            JsonRpcResponseEnvelope::Result {
                id,
                jsonrpc,
                result,
            } => {
                assert_eq!(id, 1);
                assert_eq!(jsonrpc, "2.0");
                assert_eq!(
                    serde_json::from_value::<SignerReceiveAddressResult>(result)
                        .expect("signer receive address parse")
                        .signer_receive_address,
                    "tlq1qq2xvpcvfup5j8zscjq05u2wxxjcyewk7979f3mmz5l7uw5pqmx6xf5xy50hsn6vhkm5euwt72x878eq6zxx2z58hd7zrsg9qn"
                );
            }
            other => panic!("unexpected response: {other:?}"),
        }
    }

    #[test]
    fn wallet_abi_provider_returns_raw_signing_x_only_pubkey_response() {
        let provider = make_wallet_abi_provider();

        let response_json = provider
            .process_json_rpc_request(json_rpc_request(
                2,
                GET_RAW_SIGNING_X_ONLY_PUBKEY_METHOD,
                None,
            ))
            .expect("must return json");
        let response: JsonRpcResponseEnvelope =
            serde_json::from_str(&response_json).expect("response parse");

        match response {
            JsonRpcResponseEnvelope::Result {
                id,
                jsonrpc,
                result,
            } => {
                assert_eq!(id, 2);
                assert_eq!(jsonrpc, "2.0");
                assert_eq!(
                    serde_json::from_value::<RawSigningXOnlyPubkeyResult>(result)
                        .expect("x-only pubkey parse")
                        .raw_signing_x_only_pubkey,
                    "79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798",
                );
            }
            other => panic!("unexpected response: {other:?}"),
        }
    }

    #[test]
    fn wallet_abi_provider_returns_structured_tx_create_response() {
        let provider = make_wallet_abi_provider();
        let inner_request = sample_tx_create_request();

        let response_json = provider
            .process_json_rpc_request(json_rpc_request(
                3,
                WALLET_ABI_PROCESS_REQUEST_METHOD,
                Some(serde_json::to_value(&inner_request).expect("request value")),
            ))
            .expect("must return json");
        let response: JsonRpcResponseEnvelope =
            serde_json::from_str(&response_json).expect("response parse");

        let JsonRpcResponseEnvelope::Result { id, result, .. } = response else {
            panic!("expected json-rpc result envelope");
        };
        assert_eq!(id, 3);

        let response: TxCreateResponse =
            serde_json::from_value(result).expect("inner response parse");
        assert_eq!(response.request_id, inner_request.request_id);
        assert_eq!(response.abi_version, TX_CREATE_ABI_VERSION);
        assert_eq!(response.network, inner_request.network);
        assert_eq!(response.status, Status::Error);
        assert!(response.error.is_some());
    }

    #[test]
    fn wallet_abi_provider_returns_json_rpc_error_for_removed_legacy_methods() {
        let provider = make_wallet_abi_provider();

        for method in ["wallet_abi_get_capabilities", "process_request"] {
            let response_json = provider
                .process_json_rpc_request(json_rpc_request(5, method, None))
                .expect("legacy methods return json-rpc errors");
            let response: JsonRpcResponseEnvelope =
                serde_json::from_str(&response_json).expect("response parse");

            match response {
                JsonRpcResponseEnvelope::Error { id, jsonrpc, error } => {
                    assert_eq!(id, 5);
                    assert_eq!(jsonrpc, "2.0");
                    assert_eq!(error.code, -32_601);
                    assert_eq!(error.message, format!("unsupported method '{method}'"));
                }
                other => panic!("unexpected response: {other:?}"),
            }
        }
    }

    #[test]
    fn wallet_abi_provider_returns_json_rpc_error_for_unknown_method() {
        let provider = make_wallet_abi_provider();

        let response_json = provider
            .process_json_rpc_request(json_rpc_request(6, "unsupported", None))
            .expect("unknown methods return json-rpc errors");
        let response: JsonRpcResponseEnvelope =
            serde_json::from_str(&response_json).expect("response parse");

        match response {
            JsonRpcResponseEnvelope::Error { id, jsonrpc, error } => {
                assert_eq!(id, 6);
                assert_eq!(jsonrpc, "2.0");
                assert_eq!(error.code, -32_601);
                assert_eq!(error.message, "unsupported method 'unsupported'");
            }
            other => panic!("unexpected response: {other:?}"),
        }
    }

    #[test]
    fn wallet_abi_provider_matches_core_provider_output() {
        let (signer, wallet) = make_provider_links();
        let bindings_provider = WalletAbiProvider::new(signer.clone(), wallet.clone());
        let core_provider = CoreWalletAbiProvider::new(signer, wallet);
        let request_json = json_rpc_request(
            7,
            WALLET_ABI_PROCESS_REQUEST_METHOD,
            Some(serde_json::to_value(sample_tx_create_request()).expect("request value")),
        );

        let bindings_output = bindings_provider
            .process_json_rpc_request(request_json.clone())
            .expect("bindings response");
        let core_output = core_provider
            .process_json_rpc_request(&request_json)
            .expect("core response");

        assert_eq!(bindings_output, core_output);
    }

    fn make_wallet_abi_provider() -> WalletAbiProvider {
        let (signer, wallet) = make_provider_links();
        WalletAbiProvider::new(signer, wallet)
    }

    fn make_provider_links() -> (Arc<SignerMetaLink>, Arc<WalletMetaLink>) {
        (
            Arc::new(SignerMetaLink::new(Arc::new(TestSigner))),
            Arc::new(WalletMetaLink::new(Arc::new(EmptyWallet))),
        )
    }

    fn json_rpc_request(id: i64, method: &str, params: Option<Value>) -> String {
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
            request_id: generate_request_id(),
            network: lwk_common::Network::LocaltestLiquid,
            params: RuntimeParams {
                inputs: vec![],
                outputs: vec![],
                fee_rate_sat_kvb: None,
                lock_time: None,
            },
            broadcast: false,
        }
    }
}
