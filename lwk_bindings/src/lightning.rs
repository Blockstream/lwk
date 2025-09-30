use std::sync::Arc;

use crate::{ElectrumClient, LwkError, Network};

/// A session to pay and receive lightning payments.
///
/// Lightning payments are done via LBTC swaps using Boltz.
#[derive(uniffi::Object)]
pub struct LighthingSession {
    inner: lwk_boltz::LighthingSession,
}

#[derive(uniffi::Object)]
pub struct PreparePayResponse {
    inner: lwk_boltz::PreparePayResponse,
}

#[uniffi::export]
impl LighthingSession {
    /// Create the lightning session
    ///
    /// Note the passed `ElectrumClient` should not be referenced elsewhere and it will be consumed
    /// by this method (not available after this call).
    #[uniffi::constructor]
    pub fn new(network: &Network, client: Arc<ElectrumClient>) -> Result<Self, LwkError> {
        // Try to unwrap the Arc to get owned ElectrumClient
        let inner_client = Arc::try_unwrap(client)
            .map_err(|_| LwkError::Generic {
                msg: "ElectrumClient is still referenced elsewhere".to_string(),
            })?
            .into_inner()
            .map_err(|_| LwkError::Generic {
                msg: "ElectrumClient mutex is poisoned".to_string(),
            })?;

        let client = lwk_boltz::clients::ElectrumClient::from_client(inner_client, network.into());
        let inner = lwk_boltz::LighthingSession::new(network.into(), client);
        Ok(Self { inner })
    }
}
