use std::sync::Arc;

use crate::{ExternalUtxo, Network};

/// Request-scoped wallet session record passed across the foreign callback boundary.
#[derive(uniffi::Record, Clone)]
pub struct WalletAbiRequestSession {
    /// Opaque wallet-owned request/session correlation identifier.
    pub session_id: String,
    /// Active wallet network for the request.
    pub network: Arc<Network>,
    /// Deterministic spendable wallet snapshot used for this request.
    pub spendable_utxos: Vec<Arc<ExternalUtxo>>,
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;
    use std::sync::Arc;

    use elements::Txid;

    use super::WalletAbiRequestSession;
    use crate::{ExternalUtxo, Network, OutPoint, Script, TxOut, TxOutSecrets};

    #[test]
    fn wallet_abi_request_session_holds_wallet_snapshot() {
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

        assert_eq!(session.session_id, "session-42");
        assert!(Arc::ptr_eq(&session.network, &network));
        assert_eq!(session.spendable_utxos.len(), 1);
        assert!(Arc::ptr_eq(&session.spendable_utxos[0], &utxo));
    }
}
