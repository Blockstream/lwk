use std::future::Future;
use std::sync::Arc;

use crate::wallet_abi::request_session::session_to_runtime;
use crate::{LwkError, WalletAbiRequestSession};

use lwk_simplicity::wallet_abi::WalletSessionFactory;

/// Foreign callback surface for request-scoped wallet session creation.
#[uniffi::export(with_foreign)]
pub trait WalletAbiSessionFactoryCallbacks: Send + Sync {
    /// Open one request-scoped wallet session snapshot.
    fn open_wallet_request_session(&self) -> Result<WalletAbiRequestSession, LwkError>;
}

/// Error type for the wallet session-factory bridge.
#[derive(thiserror::Error, Debug)]
pub enum WalletSessionFactoryLinkError {
    /// Error returned by the foreign callback implementation.
    #[error("{0}")]
    Foreign(String),
}

/// Bridge adapting foreign session-factory callbacks to runtime `WalletSessionFactory`.
#[derive(uniffi::Object)]
pub struct WalletSessionFactoryLink {
    inner: Arc<dyn WalletAbiSessionFactoryCallbacks>,
}

#[uniffi::export]
impl WalletSessionFactoryLink {
    /// Create a wallet session-factory bridge from foreign callback implementation.
    #[uniffi::constructor]
    pub fn new(callbacks: Arc<dyn WalletAbiSessionFactoryCallbacks>) -> Self {
        Self { inner: callbacks }
    }
}

impl WalletSessionFactory for WalletSessionFactoryLink {
    type Error = WalletSessionFactoryLinkError;

    fn open_wallet_request_session(
        &self,
    ) -> impl Future<Output = Result<lwk_simplicity::wallet_abi::WalletRequestSession, Self::Error>>
           + Send
           + '_ {
        let result = self
            .inner
            .open_wallet_request_session()
            .map(|session| session_to_runtime(&session))
            .map_err(|error| WalletSessionFactoryLinkError::Foreign(format!("{error:?}")));
        async move { result }
    }
}

#[cfg(test)]
mod tests {
    use std::future::Future;
    use std::pin::pin;
    use std::sync::Arc;
    use std::task::{Context, Poll, Waker};

    use elements::Txid;
    use std::str::FromStr;

    use super::*;
    use crate::{ExternalUtxo, Network, OutPoint, Script, TxOut, TxOutSecrets};

    struct TestSessionFactoryCallbacks {
        session: WalletAbiRequestSession,
    }

    impl WalletAbiSessionFactoryCallbacks for TestSessionFactoryCallbacks {
        fn open_wallet_request_session(&self) -> Result<WalletAbiRequestSession, LwkError> {
            Ok(self.session.clone())
        }
    }

    #[test]
    fn wallet_session_factory_link_opens_runtime_session() {
        let network = Network::regtest_default();
        let txid =
            Txid::from_str("3ac4f7d2d18e12256b4372d7947bf1df5cc640860cd63558e29cb2ec29319631")
                .expect("txid");
        let outpoint = OutPoint::from_parts(&txid.into(), 1);
        let txout = TxOut::from_explicit(&Script::empty(), network.policy_asset(), 5_000);
        let secrets = TxOutSecrets::from_explicit(network.policy_asset(), 5_000);
        let utxo = ExternalUtxo::from_unchecked_data(&outpoint, &txout, &secrets, 136);
        let session = WalletAbiRequestSession {
            session_id: "session-42".to_string(),
            network: network.clone(),
            spendable_utxos: vec![utxo.clone()],
        };
        let link = WalletSessionFactoryLink::new(Arc::new(TestSessionFactoryCallbacks { session }));

        let runtime_session =
            ready(link.open_wallet_request_session()).expect("wallet request session");

        assert_eq!(runtime_session.session_id, "session-42");
        assert_eq!(runtime_session.network, network.as_ref().into());
        assert_eq!(runtime_session.spendable_utxos.len(), 1);
        assert_eq!(runtime_session.spendable_utxos[0], utxo.as_ref().into());
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
