use crate::{ElectrumClient, LwkError, Network};

/// A session to pay and receive lightning payments.
///
/// Lightning payments are done via LBTC swaps using Boltz.
pub struct LighthingSession {
    inner: lwk_boltz::LighthingSession,
}

impl LighthingSession {
    /// Create the lightning session
    pub fn new(network: Network, client: ElectrumClient) -> Result<Self, LwkError> {
        let client = lwk_boltz::clients::ElectrumClient::from_client(
            client.into_inner().expect("todo"),
            network.into(),
        );
        let inner = lwk_boltz::LighthingSession::new(network.into(), client);
        Ok(Self { inner })
    }
}
