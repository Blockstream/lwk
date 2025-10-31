//! Clients to fetch data from the Blockchain.

use crate::{
    store::{Height, Timestamp},
    BlindingPublicKey, Chain, DownloadTxResult, ElementsNetwork, Error, WolletDescriptor, EC,
};
use elements::{
    bitcoin::bip32::ChildNumber,
    confidential::{Asset, Nonce, Value},
    AssetIssuance, LockTime, Script, Sequence, TxInWitness, TxOut, TxOutSecrets,
};
use elements::{BlockHash, OutPoint, Txid};
use lwk_common::derive_blinding_key;
use serde::Deserialize;
use std::{
    collections::{HashMap, HashSet},
    ops::{Index, IndexMut},
};

#[cfg(not(target_arch = "wasm32"))]
pub mod blocking;

pub mod asyncr;

/// A builder for the [`crate::clients::asyncr::EsploraClient`] or [`crate::clients::blocking::EsploraClient`]
#[derive(Debug, Clone)]
pub struct EsploraClientBuilder {
    base_url: String,
    waterfalls: bool,
    utxo_only: bool,
    network: ElementsNetwork,
    headers: HashMap<String, String>,
    timeout: Option<u8>,
    concurrency: Option<usize>,
    token_provider: TokenProvider,
}

/// Provider of a token for authenticated services backend of Esplora and Waterfalls
#[derive(Debug, Clone)]
pub enum TokenProvider {
    /// No token is needed
    None,
    /// A static token is used
    Static(String),
    /// A token is obtained from the Blockstream API
    Blockstream {
        /// The url to get the token from
        url: String,
        /// The client ID
        client_id: String,
        /// The client secret
        client_secret: String,
    },
}

impl EsploraClientBuilder {
    /// Create a new [`EsploraClientBuilder`]
    pub fn new(base_url: &str, network: ElementsNetwork) -> Self {
        Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            waterfalls: false,
            utxo_only: false,
            network,
            headers: HashMap::new(),
            timeout: None,
            concurrency: None,
            token_provider: TokenProvider::None,
        }
    }

    /// If `waterfalls` is true, it expects the server support the descriptor endpoint, which avoids several roundtrips
    /// during the scan and for this reason is much faster. To achieve so, the "bitcoin descriptor" part is shared with
    /// the server. All of the address are shared with the server anyway even without the waterfalls scan, but in
    /// separate calls, and in this case future addresses cannot be derived.
    /// In both cases, the server can see transactions that are involved in the wallet but it knows nothing about the
    /// assets and amount exchanged due to the nature of confidential transactions.
    pub fn waterfalls(mut self, waterfalls: bool) -> Self {
        self.waterfalls = waterfalls;
        self
    }

    /// Experimental: if true, the client will only fetch transactions with unspent outputs.
    /// The resulting balance will be the same as a full scan, but the scan will be faster
    /// at the cost of not having the full transaciton history.
    pub fn utxo_only(mut self, utxo_only: bool) -> Self {
        self.utxo_only = utxo_only;
        self
    }

    /// Set a timeout in seconds for requests
    pub fn timeout(mut self, timeout: u8) -> Self {
        self.timeout = Some(timeout);
        self
    }

    /// Set the concurrency level for requests, default is 1.
    /// Concurrency can't be 0, if 0 is passed 1 will be used.
    pub fn concurrency(mut self, concurrency: usize) -> Self {
        self.concurrency = Some(concurrency.max(1)); // 0 would hang the executor
        self
    }

    /// Set the HTTP request headers for each request
    pub fn headers(mut self, headers: HashMap<String, String>) -> Self {
        self.headers = headers;
        self
    }

    /// Add a HTTP header to set on each request
    pub fn header(mut self, key: String, val: String) -> Self {
        self.headers.insert(key, val);
        self
    }

    /// Set the token provider for authenticated services
    pub fn token_provider(mut self, token: TokenProvider) -> Self {
        self.token_provider = token;
        self
    }
}

/// Last unused derivation index for each chain.
/// In other words the next index to be used when creating a new internal or external address.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct LastUnused {
    /// The last unused internal index (for changes).
    pub internal: u32,
    /// The last unused external index.
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
    /// The new transaction ids that are involved in the update, with the confirmation height if confirmed.
    pub txid_height: HashMap<Txid, Option<Height>>,

    /// The new scripts that are involved in the update.
    pub scripts: HashMap<Script, (Chain, ChildNumber, BlindingPublicKey)>,

    /// The last unused index for each chain.
    pub last_unused: LastUnused,

    /// The block hash of the block at the given height.
    pub height_blockhash: HashMap<Height, BlockHash>,

    /// The timestamp of the block at the given height, to be used to get the timestamp of the transaction.
    pub height_timestamp: HashMap<Height, Timestamp>,

    /// The tip of the blockchain at the time the get_history was called.
    pub tip: Option<BlockHash>,

    /// The unspent outputs of this get_history call, this is non-empty only in waterfalls UTXO mode, where it's needed to know which outputs are unspent to create a dummy tx spending all the other outputs.
    pub unspent: Vec<OutPoint>,
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

    /// The 1-based index of the output of the transaction. available only in utxo scan, otherwise 0.
    #[serde(default)]
    pub v: i32,
}

/// Try to unblind a transaction output using the given descriptor
pub fn try_unblind(output: &TxOut, descriptor: &WolletDescriptor) -> Result<TxOutSecrets, Error> {
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

#[allow(unused)]
pub(crate) fn check_witnesses_non_empty(tx: &elements::Transaction) -> Result<(), Error> {
    if tx.input.iter().any(|e| e.witness.is_empty()) {
        return Err(Error::EmptyWitness);
    }
    Ok(())
}

// Creates a dummy tx having inputs spending all the outputs of the download transactions which are not unspent.
//
// We may need to return a vec of transactions if some kind of transaction limits arise.
// TODO: Add only outpoints the wallet owns.
fn create_dummy_tx(unspent: &[OutPoint], new_txs: &DownloadTxResult) -> elements::Transaction {
    let mut all_outputs: HashSet<OutPoint> = new_txs
        .txs
        .iter()
        .flat_map(|(txid, tx)| {
            tx.output
                .iter()
                .enumerate()
                .map(|(i, _)| OutPoint::new(*txid, i as u32))
        })
        .collect();
    all_outputs.retain(|o| !unspent.contains(o));
    let spent_outputs = all_outputs;

    let inputs = spent_outputs
        .iter()
        .map(|o| elements::TxIn {
            previous_output: *o,
            script_sig: elements::Script::default(),
            sequence: Sequence::MAX,
            is_pegin: false,
            asset_issuance: AssetIssuance::default(),
            witness: TxInWitness::default(),
        })
        .collect();

    let outputs = vec![];

    elements::Transaction {
        version: 1,
        input: inputs,
        output: outputs,
        lock_time: LockTime::ZERO,
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
