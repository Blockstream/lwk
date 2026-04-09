use std::future::Future;
use std::sync::Arc;

use crate::{LwkError, Transaction, Txid};

use lwk_simplicity::wallet_abi::WalletBroadcaster;

/// Foreign callback surface for transaction broadcasting.
#[uniffi::export(with_foreign)]
pub trait WalletAbiBroadcasterCallbacks: Send + Sync {
    /// Broadcast finalized transaction and return backend-reported txid.
    fn broadcast_transaction(&self, tx: Arc<Transaction>) -> Result<Arc<Txid>, LwkError>;
}

/// Error type for the wallet broadcaster bridge.
#[derive(thiserror::Error, Debug)]
pub enum WalletBroadcasterLinkError {
    /// Error returned by the foreign callback implementation.
    #[error("{0}")]
    Foreign(String),
}

/// Bridge adapting foreign broadcaster callbacks to runtime `WalletBroadcaster`.
#[derive(uniffi::Object)]
pub struct WalletBroadcasterLink {
    inner: Arc<dyn WalletAbiBroadcasterCallbacks>,
}

#[uniffi::export]
impl WalletBroadcasterLink {
    /// Create a wallet broadcaster bridge from foreign callback implementation.
    #[uniffi::constructor]
    pub fn new(callbacks: Arc<dyn WalletAbiBroadcasterCallbacks>) -> Self {
        Self { inner: callbacks }
    }
}

impl WalletBroadcaster for WalletBroadcasterLink {
    type Error = WalletBroadcasterLinkError;

    fn broadcast_transaction(
        &self,
        tx: &elements::Transaction,
    ) -> impl Future<Output = Result<elements::Txid, Self::Error>> + Send + '_ {
        let result = self
            .inner
            .broadcast_transaction(Arc::new(tx.clone().into()))
            .map(|txid| txid.as_ref().into())
            .map_err(|error| WalletBroadcasterLinkError::Foreign(format!("{error:?}")));
        async move { result }
    }
}

#[cfg(test)]
mod tests {
    use std::future::Future;
    use std::pin::pin;
    use std::str::FromStr;
    use std::sync::Arc;
    use std::task::{Context, Poll, Waker};

    use super::*;

    struct TestBroadcasterCallbacks {
        txid: Arc<Txid>,
    }

    impl WalletAbiBroadcasterCallbacks for TestBroadcasterCallbacks {
        fn broadcast_transaction(&self, tx: Arc<Transaction>) -> Result<Arc<Txid>, LwkError> {
            assert_eq!(
                tx.to_string(),
                "0200000000010000000000000000000000000000000000000000000000000000000000000000ffffffff00ffffffff0000000000"
            );
            Ok(self.txid.clone())
        }
    }

    #[test]
    fn wallet_broadcaster_link_adapts_foreign_callbacks() {
        let txid = Arc::new(
            Txid::from_str("3ac4f7d2d18e12256b4372d7947bf1df5cc640860cd63558e29cb2ec29319631")
                .expect("txid"),
        );
        let link = WalletBroadcasterLink::new(Arc::new(TestBroadcasterCallbacks {
            txid: txid.clone(),
        }));
        let tx = elements::Transaction {
            version: 2,
            lock_time: elements::LockTime::ZERO,
            input: vec![elements::TxIn::default()],
            output: vec![],
        };

        assert_eq!(
            ready(link.broadcast_transaction(&tx)).expect("broadcast txid"),
            txid.as_ref().into()
        );
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
