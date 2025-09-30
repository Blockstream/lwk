use crate::{ElectrumClient, LwkError, Network};

pub struct LighthingSession {
    inner: lwk_boltz::LighthingSession,
}

impl LighthingSession {
    pub fn new(network: Network, client: ElectrumClient) -> Result<Self, LwkError> {
        let client = lwk_boltz::clients::ElectrumClient::from_client(
            client.into_inner().expect("todo"),
            network.into(),
        );
        let inner = lwk_boltz::LighthingSession::new(network.into(), client);
        Ok(Self { inner })
    }
}
