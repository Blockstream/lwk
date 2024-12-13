use crate::{
    store::{Height, Timestamp},
    Chain, Error, WolletDescriptor, EC,
};
use elements::{
    bitcoin::bip32::ChildNumber,
    confidential::{Asset, Nonce, Value},
    Script, TxOut, TxOutSecrets,
};
use elements::{BlockHash, Txid};
use lwk_common::derive_blinding_key;
use serde::Deserialize;
use std::{
    collections::HashMap,
    ops::{Index, IndexMut},
};

#[cfg(not(target_arch = "wasm32"))]
pub mod blocking;

pub mod asyncr;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct LastUnused {
    pub internal: u32,
    pub external: u32,
}

impl Index<Chain> for LastUnused {
    type Output = u32;

    fn index(&self, index: Chain) -> &Self::Output {
        match index {
            Chain::External => &self.external,
            Chain::Internal => &self.internal,
        }
    }
}

impl IndexMut<Chain> for LastUnused {
    fn index_mut(&mut self, index: Chain) -> &mut Self::Output {
        match index {
            Chain::External => &mut self.external,
            Chain::Internal => &mut self.internal,
        }
    }
}

/// Data processed after a "get history" call
#[derive(Debug, PartialEq, Eq, Default)]
pub struct Data {
    pub txid_height: HashMap<Txid, Option<Height>>,
    pub scripts: HashMap<Script, (Chain, ChildNumber)>,
    pub last_unused: LastUnused,
    pub height_blockhash: HashMap<Height, BlockHash>,
    pub height_timestamp: HashMap<Height, Timestamp>,
    pub tip: Option<BlockHash>,
}

/// Capabilities that can be supported by a [`blocking::BlockchainBackend`]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Capability {
    /// Can interfact with a Waterfalls data source
    Waterfalls,
}

#[derive(Debug, Clone, Deserialize)]
/// Position of a transaction involving a certain script
pub struct History {
    /// Transaction ID
    pub txid: Txid,

    /// Confirmation height of txid
    ///
    /// -1 means unconfirmed with unconfirmed parents
    ///  0 means unconfirmed with confirmed parents
    pub height: i32,

    /// The block hash of the block including the transaction, if available
    pub block_hash: Option<BlockHash>,

    /// The block hash of the block including the transaction, if available
    pub block_timestamp: Option<Timestamp>,
}

pub fn try_unblind(output: TxOut, descriptor: &WolletDescriptor) -> Result<TxOutSecrets, Error> {
    match (output.asset, output.value, output.nonce) {
        (Asset::Confidential(_), Value::Confidential(_), Nonce::Confidential(_)) => {
            let receiver_sk = derive_blinding_key(descriptor.as_ref(), &output.script_pubkey)
                .ok_or_else(|| Error::MissingPrivateBlindingKey)?;
            let txout_secrets = output.unblind(&EC, receiver_sk)?;

            Ok(txout_secrets)
        }
        _ => Err(Error::Generic(
            "received unconfidential or null asset/value/nonce".into(),
        )),
    }
}

#[cfg(test)]
mod tests {
    /*
    use std::time::Instant;

    use crate::{
        clients::esplora_client::EsploraClient, BlockchainBackend, ElectrumClient, ElectrumUrl,
        ElementsNetwork,
    };

    #[test]
    #[ignore = "test with prod servers"]
    fn esplora_electrum_compare() {
        lwk_test_util::init_logging();

        let desc_str = lwk_test_util::TEST_DESCRIPTOR;

        let urls = [
            LIQUID_TESTNET_SOCKET,
            "https://blockstream.info/liquidtestnet/api",
            "https://liquid.network/liquidtestnet/api",
        ];

        let vec: Vec<Box<dyn BlockchainBackend>> = vec![
            Box::new(ElectrumClient::new(&ElectrumUrl::new(urls[0], true, true)).unwrap()),
            Box::new(EsploraClient::new(urls[1])),
            Box::new(EsploraClient::new(urls[2])),
        ];

        let mut prec = None;

        for (i, mut bb) in vec.into_iter().enumerate() {
            let tempdir = tempfile::tempdir().unwrap();
            let desc = desc_str.parse().unwrap();
            let mut wollet =
                crate::Wollet::with_fs_persist(ElementsNetwork::LiquidTestnet, desc, &tempdir)
                    .unwrap();

            let start = Instant::now();
            let first_update = bb.full_scan(&wollet).unwrap().unwrap();
            wollet.apply_update(first_update.clone()).unwrap();

            let balance = wollet.balance().unwrap();

            if let Some(prec) = prec.as_ref() {
                assert_eq!(&balance, prec);
            }
            prec = Some(balance);

            log::info!(
                "first run: {}: {:.2}s",
                urls[i],
                start.elapsed().as_secs_f64()
            );

            let start = Instant::now();
            let second_update = bb.full_scan(&wollet.state()).unwrap();
            if let Some(update) = second_update {
                // the tip could have been updated, checking no new tx have been found
                assert!(update.new_txs.unblinds.is_empty());
                assert!(update.scripts.is_empty());
                assert!(update.timestamps.is_empty());
                assert!(update.txid_height_new.is_empty());
                assert!(update.txid_height_delete.is_empty());
                assert_ne!(update.tip, first_update.tip);
            }
            log::info!(
                "second run: {}: {:.2}s",
                urls[i],
                start.elapsed().as_secs_f64()
            );
        }
    }
    * */
}
