use std::sync::Arc;

use crate::wallet_abi::*;

/// A typed Wallet ABI transaction evaluation request.
#[derive(uniffi::Object, Clone)]
pub struct WalletAbiTxEvaluateRequest {
    pub(crate) inner: abi::TxEvaluateRequest,
}

#[uniffi::export]
impl WalletAbiTxEvaluateRequest {
    /// Build a transaction evaluation request.
    ///
    /// `request_id` must be a valid UUID string.
    #[uniffi::constructor]
    pub fn from_parts(
        request_id: &str,
        network: &Network,
        params: &WalletAbiRuntimeParams,
    ) -> Result<Arc<Self>, LwkError> {
        Ok(Arc::new(Self {
            inner: abi::TxEvaluateRequest::from_parts(
                request_id,
                network.into(),
                params.inner.clone(),
            )?,
        }))
    }

    /// Parse canonical Wallet ABI evaluation request JSON.
    #[uniffi::constructor]
    pub fn from_json(json: &str) -> Result<Arc<Self>, LwkError> {
        Ok(Arc::new(Self {
            inner: serde_json::from_str(json)?,
        }))
    }

    /// Serialize this evaluation request to canonical Wallet ABI JSON.
    pub fn to_json(&self) -> Result<String, LwkError> {
        Ok(serde_json::to_string(&self.inner)?)
    }

    /// Return the ABI version string.
    pub fn abi_version(&self) -> String {
        self.inner.abi_version.clone()
    }

    /// Return the request identifier as a UUID string.
    pub fn request_id(&self) -> String {
        self.inner.request_id.to_string()
    }

    /// Return the target network.
    pub fn network(&self) -> Arc<Network> {
        Arc::new(self.inner.network.into())
    }

    /// Return the runtime parameters payload.
    pub fn params(&self) -> Arc<WalletAbiRuntimeParams> {
        Arc::new(WalletAbiRuntimeParams {
            inner: self.inner.params.clone(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::WalletAbiTxEvaluateRequest;
    use crate::{Network, WalletAbiRuntimeParams};

    #[test]
    fn wallet_abi_tx_evaluate_request_roundtrip() {
        let request = WalletAbiTxEvaluateRequest::from_parts(
            "0d6d53cd-a040-4f0c-8d28-c67b6608fb14",
            &Network::testnet(),
            &WalletAbiRuntimeParams::new(&[], &[], None, None),
        )
        .expect("request");

        let json = request.to_json().expect("serialize request");
        let decoded =
            WalletAbiTxEvaluateRequest::from_json(&json).expect("deserialize request");

        assert_eq!(decoded.abi_version(), "wallet-abi-0.1");
        assert_eq!(
            decoded.request_id(),
            "0d6d53cd-a040-4f0c-8d28-c67b6608fb14".to_string()
        );
        assert_eq!(decoded.network(), Network::testnet());
        assert!(decoded.params().inputs().is_empty());
        assert!(decoded.params().outputs().is_empty());
        assert_eq!(decoded.params().fee_rate_sat_kvb(), None);
        assert_eq!(decoded.params().lock_time(), None);
    }
}
