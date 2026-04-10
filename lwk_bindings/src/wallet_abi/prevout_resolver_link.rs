use std::future::Future;
use std::str::FromStr;
use std::sync::Arc;

use crate::{LwkError, OutPoint, TxOut, TxOutSecrets, WalletAbiBip32DerivationPair};

use elements::bitcoin::bip32::{DerivationPath, Fingerprint, KeySource};
use elements::bitcoin::PublicKey;
use lwk_simplicity::wallet_abi::WalletPrevoutResolver;

/// Foreign callback surface for prevout fetch, wallet unblinding, and BIP32 metadata.
#[uniffi::export(with_foreign)]
pub trait WalletAbiPrevoutResolverCallbacks: Send + Sync {
    /// Return wallet-owned BIP32 derivation metadata for the selected outpoint.
    fn get_bip32_derivation_pair(
        &self,
        outpoint: Arc<OutPoint>,
    ) -> Result<Option<WalletAbiBip32DerivationPair>, LwkError>;

    /// Unblind one output using wallet-owned descriptor or blinding material.
    fn unblind(&self, tx_out: Arc<TxOut>) -> Result<Arc<TxOutSecrets>, LwkError>;

    /// Fetch previous output by outpoint.
    fn get_tx_out(&self, outpoint: Arc<OutPoint>) -> Result<Arc<TxOut>, LwkError>;
}

/// Error type for the wallet prevout-resolver bridge.
#[derive(thiserror::Error, Debug)]
pub enum WalletPrevoutResolverLinkError {
    /// Error returned by the foreign callback implementation.
    #[error("{0}")]
    Foreign(String),
    /// The foreign callback returned a derivation pair that could not be parsed.
    #[error("invalid wallet-owned BIP32 derivation pair: {0}")]
    InvalidDerivationPair(String),
}

/// Bridge adapting foreign prevout-resolver callbacks to runtime `WalletPrevoutResolver`.
#[derive(uniffi::Object)]
pub struct WalletPrevoutResolverLink {
    inner: Arc<dyn WalletAbiPrevoutResolverCallbacks>,
}

#[uniffi::export]
impl WalletPrevoutResolverLink {
    /// Create a wallet prevout-resolver bridge from foreign callback implementation.
    #[uniffi::constructor]
    pub fn new(callbacks: Arc<dyn WalletAbiPrevoutResolverCallbacks>) -> Self {
        Self { inner: callbacks }
    }
}

impl WalletPrevoutResolver for WalletPrevoutResolverLink {
    type Error = WalletPrevoutResolverLinkError;

    fn get_bip32_derivation_pair(
        &self,
        out_point: &elements::OutPoint,
    ) -> Result<Option<(PublicKey, KeySource)>, Self::Error> {
        self.inner
            .get_bip32_derivation_pair(Arc::new((*out_point).into()))
            .map_err(|error| WalletPrevoutResolverLinkError::Foreign(format!("{error:?}")))?
            .map(derivation_pair_from_binding)
            .transpose()
    }

    fn unblind(&self, tx_out: &elements::TxOut) -> Result<elements::TxOutSecrets, Self::Error> {
        self.inner
            .unblind(Arc::new(tx_out.clone().into()))
            .map(|secrets| secrets.as_ref().into())
            .map_err(|error| WalletPrevoutResolverLinkError::Foreign(format!("{error:?}")))
    }

    fn get_tx_out(
        &self,
        outpoint: elements::OutPoint,
    ) -> impl Future<Output = Result<elements::TxOut, Self::Error>> + Send + '_ {
        let result = self
            .inner
            .get_tx_out(Arc::new(outpoint.into()))
            .map(|tx_out| tx_out.as_ref().into())
            .map_err(|error| WalletPrevoutResolverLinkError::Foreign(format!("{error:?}")));
        async move { result }
    }
}

fn derivation_pair_from_binding(
    pair: WalletAbiBip32DerivationPair,
) -> Result<(PublicKey, KeySource), WalletPrevoutResolverLinkError> {
    let (fingerprint, derivation_path) = pair
        .key_source
        .strip_prefix('[')
        .and_then(|key_source| key_source.split_once(']'))
        .ok_or_else(|| {
            WalletPrevoutResolverLinkError::InvalidDerivationPair(pair.key_source.clone())
        })?;

    let fingerprint = Fingerprint::from_str(fingerprint).map_err(|error| {
        WalletPrevoutResolverLinkError::InvalidDerivationPair(error.to_string())
    })?;
    let derivation_path =
        DerivationPath::from_str(&format!("m/{derivation_path}")).map_err(|error| {
            WalletPrevoutResolverLinkError::InvalidDerivationPair(error.to_string())
        })?;
    let pubkey = PublicKey::from_str(&pair.pubkey).map_err(|error| {
        WalletPrevoutResolverLinkError::InvalidDerivationPair(error.to_string())
    })?;

    Ok((pubkey, (fingerprint, derivation_path)))
}

#[cfg(test)]
mod tests {
    use std::future::Future;
    use std::pin::pin;
    use std::str::FromStr;
    use std::sync::Arc;
    use std::task::{Context, Poll, Waker};

    use elements::Txid;

    use super::*;
    use crate::{Network, Script};

    struct TestPrevoutResolverCallbacks {
        derivation_pair: WalletAbiBip32DerivationPair,
        tx_out: Arc<TxOut>,
        secrets: Arc<TxOutSecrets>,
    }

    impl WalletAbiPrevoutResolverCallbacks for TestPrevoutResolverCallbacks {
        fn get_bip32_derivation_pair(
            &self,
            _outpoint: Arc<OutPoint>,
        ) -> Result<Option<WalletAbiBip32DerivationPair>, LwkError> {
            Ok(Some(self.derivation_pair.clone()))
        }

        fn unblind(&self, _tx_out: Arc<TxOut>) -> Result<Arc<TxOutSecrets>, LwkError> {
            Ok(self.secrets.clone())
        }

        fn get_tx_out(&self, _outpoint: Arc<OutPoint>) -> Result<Arc<TxOut>, LwkError> {
            Ok(self.tx_out.clone())
        }
    }

    #[test]
    fn wallet_prevout_resolver_link_adapts_foreign_callbacks() {
        let network = Network::regtest_default();
        let txid =
            Txid::from_str("3ac4f7d2d18e12256b4372d7947bf1df5cc640860cd63558e29cb2ec29319631")
                .expect("txid");
        let outpoint = elements::OutPoint::new(txid, 1);
        let tx_out = TxOut::from_explicit(&Script::empty(), network.policy_asset(), 5_000);
        let secrets = TxOutSecrets::from_explicit(network.policy_asset(), 5_000);
        let link = WalletPrevoutResolverLink::new(Arc::new(TestPrevoutResolverCallbacks {
            derivation_pair: WalletAbiBip32DerivationPair {
                pubkey: "0279be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798"
                    .to_string(),
                key_source: "[73c5da0a]84'/1'/0'/0/5".to_string(),
            },
            tx_out: tx_out.clone(),
            secrets: secrets.clone(),
        }));

        let derivation_pair = link
            .get_bip32_derivation_pair(&outpoint)
            .expect("derivation pair")
            .expect("present");

        assert_eq!(
            derivation_pair,
            (
                PublicKey::from_str(
                    "0279be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798"
                )
                .expect("pubkey"),
                (
                    Fingerprint::from_str("73c5da0a").expect("fingerprint"),
                    DerivationPath::from_str("m/84'/1'/0'/0/5").expect("path"),
                ),
            )
        );
        let tx_out_inner: elements::TxOut = tx_out.as_ref().into();
        assert_eq!(
            link.unblind(&tx_out_inner).expect("unblind"),
            elements::TxOutSecrets::from(secrets.as_ref())
        );
        assert_eq!(
            ready(link.get_tx_out(outpoint)).expect("tx out"),
            elements::TxOut::from(tx_out.as_ref())
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
