use crate::wallet_abi::*;

#[derive(uniffi::Enum, Clone, Copy, Debug, PartialEq, Eq)]
/// The status of a Wallet ABI transaction creation response.
pub enum WalletAbiStatus {
    /// The request succeeded.
    Ok,
    /// The request failed.
    Error,
}

#[derive(uniffi::Enum, Clone, Copy, Debug, PartialEq, Eq)]
/// A Wallet ABI error code.
pub enum WalletAbiErrorCode {
    /// The request payload is invalid.
    InvalidRequest,
    /// JSON serialization or deserialization failed.
    Serde,
    /// Program execution failed.
    ProgramError,
    /// Key or path derivation failed.
    Derivation,
    /// Integer conversion failed.
    TryFromInt,
    /// Funding or coin selection failed.
    Funding,
    /// The signer configuration is invalid.
    InvalidSignerConfig,
    /// The response payload is invalid.
    InvalidResponse,
    /// PSET construction failed.
    Pset,
    /// PSET blinding failed.
    PsetBlind,
    /// Amount proof verification failed.
    AmountProofVerification,
    /// Finalization steps are invalid.
    InvalidFinalizationSteps,
    /// An unknown error code was reported.
    Unknown,
}

/// A created transaction payload returned by Wallet ABI.
#[derive(uniffi::Object, Clone)]
pub struct WalletAbiTransactionInfo {
    pub(crate) inner: abi::TransactionInfo,
}

#[uniffi::export]
impl WalletAbiTransactionInfo {
    /// Build transaction info from transaction hex and txid.
    #[uniffi::constructor]
    pub fn new(tx_hex: &str, txid: &Txid) -> Arc<Self> {
        Arc::new(Self {
            inner: abi::TransactionInfo {
                tx_hex: tx_hex.to_string(),
                txid: txid.into(),
            },
        })
    }

    /// Return the transaction hex.
    pub fn tx_hex(&self) -> String {
        self.inner.tx_hex.clone()
    }

    /// Return the transaction id.
    pub fn txid(&self) -> Arc<Txid> {
        Arc::new(self.inner.txid.into())
    }
}

/// Error details returned by Wallet ABI.
#[derive(uniffi::Object, Clone)]
pub struct WalletAbiErrorInfo {
    pub(crate) inner: abi::ErrorInfo,
}

#[uniffi::export]
impl WalletAbiErrorInfo {
    /// Build error info from a canonical Wallet ABI error code string.
    #[uniffi::constructor]
    pub fn from_code_string(
        code: &str,
        message: &str,
        details_json: Option<String>,
    ) -> Result<Arc<Self>, LwkError> {
        Ok(Arc::new(Self {
            inner: abi::ErrorInfo::from_code_and_json(code, message, details_json.as_deref())?,
        }))
    }

    /// Return the parsed error code enum.
    pub fn code(&self) -> WalletAbiErrorCode {
        (&self.inner.code).into()
    }

    /// Return the canonical Wallet ABI error code string.
    pub fn code_string(&self) -> String {
        self.inner.code.as_str().to_string()
    }

    /// Return the human-readable error message.
    pub fn message(&self) -> String {
        self.inner.message.clone()
    }

    /// Returns canonical JSON for the open-ended `details` payload.
    pub fn details_json(&self) -> Result<Option<String>, LwkError> {
        self.inner.details_json().map_err(Into::into)
    }
}

/// A typed Wallet ABI transaction creation request.
#[derive(uniffi::Object, Clone)]
pub struct WalletAbiTxCreateRequest {
    pub(crate) inner: abi::TxCreateRequest,
}

#[uniffi::export]
impl WalletAbiTxCreateRequest {
    /// Build a transaction creation request.
    ///
    /// `request_id` must be a valid UUID string.
    #[uniffi::constructor]
    pub fn from_parts(
        request_id: &str,
        network: &Network,
        params: &WalletAbiRuntimeParams,
        broadcast: bool,
    ) -> Result<Arc<Self>, LwkError> {
        Ok(Arc::new(Self {
            inner: abi::TxCreateRequest::from_parts(
                request_id,
                network.into(),
                params.inner.clone(),
                broadcast,
            )?,
        }))
    }

    /// Parse canonical Wallet ABI request JSON.
    #[uniffi::constructor]
    pub fn from_json(json: &str) -> Result<Arc<Self>, LwkError> {
        Ok(Arc::new(Self {
            inner: serde_json::from_str(json)?,
        }))
    }

    /// Serialize this request to canonical Wallet ABI JSON.
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
    pub fn params(&self) -> Arc<crate::WalletAbiRuntimeParams> {
        Arc::new(crate::WalletAbiRuntimeParams {
            inner: self.inner.params.clone(),
        })
    }

    /// Return whether the request asks for broadcast.
    pub fn broadcast(&self) -> bool {
        self.inner.broadcast
    }
}

/// A typed Wallet ABI transaction creation response.
#[derive(uniffi::Object, Clone)]
pub struct WalletAbiTxCreateResponse {
    pub(crate) inner: abi::TxCreateResponse,
}

#[uniffi::export]
impl WalletAbiTxCreateResponse {
    /// Build a successful transaction creation response.
    ///
    /// `request_id` must be a valid UUID string.
    #[uniffi::constructor]
    pub fn ok(
        request_id: &str,
        network: &Network,
        transaction: &WalletAbiTransactionInfo,
        artifacts_json: Option<String>,
    ) -> Result<Arc<Self>, LwkError> {
        Ok(Arc::new(Self {
            inner: abi::TxCreateResponse::ok_from_parts(
                request_id,
                network.into(),
                transaction.inner.clone(),
                artifacts_json.as_deref(),
            )?,
        }))
    }

    /// Build an error transaction creation response.
    ///
    /// `request_id` must be a valid UUID string.
    #[uniffi::constructor]
    pub fn error(
        request_id: &str,
        network: &Network,
        error: &WalletAbiErrorInfo,
    ) -> Result<Arc<Self>, LwkError> {
        Ok(Arc::new(Self {
            inner: abi::TxCreateResponse::error_from_parts(
                request_id,
                network.into(),
                error.inner.clone(),
            )?,
        }))
    }

    /// Parse canonical Wallet ABI response JSON.
    #[uniffi::constructor]
    pub fn from_json(json: &str) -> Result<Arc<Self>, LwkError> {
        Ok(Arc::new(Self {
            inner: serde_json::from_str(json)?,
        }))
    }

    /// Serialize this response to canonical Wallet ABI JSON.
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

    /// Return the response status.
    pub fn status(&self) -> WalletAbiStatus {
        self.inner.status.into()
    }

    /// Return the transaction when this response has `ok` status.
    pub fn transaction(&self) -> Option<Arc<WalletAbiTransactionInfo>> {
        self.inner.transaction.as_ref().map(|transaction| {
            Arc::new(WalletAbiTransactionInfo {
                inner: transaction.clone(),
            })
        })
    }

    /// Returns canonical JSON for the open-ended `artifacts` payload.
    pub fn artifacts_json(&self) -> Result<Option<String>, LwkError> {
        self.inner.artifacts_json().map_err(Into::into)
    }

    /// Return the typed preview payload when `artifacts.preview` is present.
    pub fn preview(&self) -> Result<Option<Arc<WalletAbiRequestPreview>>, LwkError> {
        self.inner
            .preview()
            .map(|preview| preview.map(|inner| Arc::new(WalletAbiRequestPreview { inner })))
            .map_err(Into::into)
    }

    /// Return the error payload when this response has `error` status.
    pub fn error_info(&self) -> Option<Arc<WalletAbiErrorInfo>> {
        self.inner.error.as_ref().map(|error| {
            Arc::new(WalletAbiErrorInfo {
                inner: error.clone(),
            })
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{WalletAbiTransactionInfo, WalletAbiTxCreateResponse};
    use crate::blockdata::script::Script;
    use crate::{
        Network, WalletAbiPreviewAssetDelta, WalletAbiPreviewOutput, WalletAbiPreviewOutputKind,
        WalletAbiRequestPreview,
    };

    #[test]
    fn wallet_abi_tx_create_response_preview() {
        let network = Network::testnet();
        let policy_asset = network.policy_asset();
        let preview = WalletAbiRequestPreview::new(
            vec![WalletAbiPreviewAssetDelta::new(policy_asset, -1_500)],
            vec![WalletAbiPreviewOutput::new(
                WalletAbiPreviewOutputKind::Fee,
                policy_asset,
                600,
                &Script::empty(),
            )],
            vec![],
        );
        let transaction = WalletAbiTransactionInfo::new(
            "00",
            &"0000000000000000000000000000000000000000000000000000000000000000"
                .parse()
                .expect("valid txid"),
        );
        let mut artifacts = serde_json::Map::new();
        artifacts.insert(
            "preview".to_string(),
            serde_json::from_str::<serde_json::Value>(
                &preview.to_json().expect("serialize preview"),
            )
            .expect("preview json value"),
        );
        let response = WalletAbiTxCreateResponse::ok(
            "0d6d53cd-a040-4f0c-8d28-c67b6608fb14",
            &network,
            &transaction,
            Some(serde_json::to_string(&artifacts).expect("serialize artifacts")),
        )
        .expect("response");

        let decoded = response
            .preview()
            .expect("preview accessor")
            .expect("preview payload");

        assert_eq!(decoded.asset_deltas()[0].wallet_delta_sat(), -1_500);
        assert_eq!(decoded.outputs()[0].kind(), WalletAbiPreviewOutputKind::Fee);
    }
}
