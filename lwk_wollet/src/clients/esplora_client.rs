use std::{
    collections::{HashMap, HashSet},
    str::FromStr,
};

use age::x25519::Recipient;
use elements::{
    bitcoin::bip32::ChildNumber,
    encode::Decodable,
    hashes::{hex::FromHex, sha256, Hash},
    hex::ToHex,
    pset::serialize::Serialize,
    BlockHash, Script, Txid,
};
use elements_miniscript::DescriptorPublicKey;
use reqwest::blocking::{self, Response};
use serde::Deserialize;

use crate::{
    clients::waterfalls::{encrypt, WaterfallsResult},
    clients::{Capability, Data, History},
    store::Height,
    wollet::WolletState,
    BlockchainBackend, Chain, Error, WolletDescriptor,
};

#[derive(Debug)]
/// A blockchain backend implementation based on the
/// [esplora HTTP API](https://github.com/blockstream/esplora/blob/master/API.md)
pub struct EsploraClient {
    client: blocking::Client,
    base_url: String,
    tip_hash_url: String,
    broadcast_url: String,

    waterfalls: bool,
    waterfalls_server_recipient: Option<Recipient>,
    waterfalls_avoid_encryption: bool,
}

impl EsploraClient {
    pub fn new(url: &str) -> Self {
        Self {
            client: blocking::Client::new(),
            base_url: url.to_string(),
            tip_hash_url: format!("{url}/blocks/tip/hash"),
            broadcast_url: format!("{url}/tx"),
            waterfalls: false,
            waterfalls_server_recipient: None,
            waterfalls_avoid_encryption: false,
        }
    }

    fn last_block_hash(&mut self) -> Result<elements::BlockHash, crate::Error> {
        let response = get_with_retry(&self.tip_hash_url, 0)?;
        Ok(BlockHash::from_str(&response.text()?)?)
    }
}

/// "Waterfalls" methods
impl EsploraClient {
    /// Create a new Esplora client using the "waterfalls" endpoint
    pub fn new_waterfalls(url: &str) -> Self {
        let mut client = Self::new(url);
        client.waterfalls = true;
        client
    }

    /// Do not encrypt the descriptor when using the "waterfalls" endpoint
    pub fn waterfalls_avoid_encryption(&mut self) {
        self.waterfalls_avoid_encryption = true;
    }

    fn waterfalls_server_recipient(&mut self) -> Result<Recipient, Error> {
        match self.waterfalls_server_recipient.as_ref() {
            Some(r) => Ok(r.clone()),
            None => {
                let url = format!("{}/v1/server_recipient", self.base_url);
                let response = self.client.get(url).send()?;
                let rec = Recipient::from_str(&response.text()?)
                    .map_err(|_| Error::CannotParseRecipientKey)?;
                self.waterfalls_server_recipient = Some(rec.clone());
                Ok(rec)
            }
        }
    }
}

impl BlockchainBackend for EsploraClient {
    fn tip(&mut self) -> Result<elements::BlockHeader, crate::Error> {
        let last_block_hash = self.last_block_hash()?;

        let header_url = format!("{}/block/{}/header", self.base_url, last_block_hash);
        let response = get_with_retry(&header_url, 0)?;
        let header_bytes = Vec::<u8>::from_hex(&response.text()?)?;

        let header = elements::BlockHeader::consensus_decode(&header_bytes[..])?;
        Ok(header)
    }

    fn broadcast(&self, tx: &elements::Transaction) -> Result<elements::Txid, crate::Error> {
        let tx_hex = tx.serialize().to_hex();
        let response = self.client.post(&self.broadcast_url).body(tx_hex).send()?;
        let txid = elements::Txid::from_str(&response.text()?)?;
        Ok(txid)
    }

    fn get_transactions(&self, txids: &[Txid]) -> Result<Vec<elements::Transaction>, Error> {
        let mut result = vec![];
        for txid in txids.iter() {
            let tx_url = format!("{}/tx/{}/raw", self.base_url, txid);
            let response = get_with_retry(&tx_url, 0)?;
            let tx = elements::Transaction::consensus_decode(&response.bytes()?[..])?;
            result.push(tx);
        }
        Ok(result)
    }

    fn get_headers(
        &self,
        heights: &[Height],
        height_blockhash: &HashMap<Height, BlockHash>,
    ) -> Result<Vec<elements::BlockHeader>, Error> {
        let mut result = vec![];
        for height in heights.iter() {
            let block_hash = match height_blockhash.get(height) {
                Some(block_hash) => *block_hash,
                None => {
                    let block_height = format!("{}/block-height/{}", self.base_url, height);
                    let response = get_with_retry(&block_height, 0)?;
                    BlockHash::from_str(&response.text()?)?
                }
            };

            let block_header = format!("{}/block/{}/header", self.base_url, block_hash);
            let response = get_with_retry(&block_header, 0)?;
            let header_bytes = Vec::<u8>::from_hex(&response.text()?)?;

            let header = elements::BlockHeader::consensus_decode(&header_bytes[..])?;

            result.push(header);
        }
        Ok(result)
    }

    // examples:
    // https://blockstream.info/liquidtestnet/api/address/tex1qntw9m0j2e93n84x975t47ddhgkzx3x8lhfv2nj/txs
    // https://blockstream.info/liquidtestnet/api/scripthash/b50a2a798d876db54acfa0d8dfdc49154ea8defed37b225ec4c9ec7415358ba3/txs
    fn get_scripts_history(&self, scripts: &[&Script]) -> Result<Vec<Vec<History>>, Error> {
        let mut result: Vec<_> = vec![];
        for script in scripts.iter() {
            let script = elements::bitcoin::Script::from_bytes(script.as_bytes());
            let script_hash = sha256::Hash::hash(script.as_bytes()).to_byte_array();
            let url = format!("{}/scripthash/{}/txs", self.base_url, script_hash.to_hex());
            // TODO must handle paging -> https://github.com/blockstream/esplora/blob/master/API.md#addresses
            let response = get_with_retry(&url, 0)?;
            let json: Vec<EsploraTx> = response.json()?;

            let history: Vec<History> = json.into_iter().map(Into::into).collect();
            result.push(history)
        }
        Ok(result)
    }

    fn capabilities(&self) -> HashSet<Capability> {
        if self.waterfalls {
            vec![Capability::Waterfalls].into_iter().collect()
        } else {
            HashSet::new()
        }
    }

    fn get_history_waterfalls<S: WolletState>(
        &mut self,
        descriptor: &WolletDescriptor,
        state: &S,
    ) -> Result<Data, Error> {
        let descriptor_url = format!("{}/v1/waterfalls", self.base_url);
        if descriptor.is_elip151() {
            return Err(Error::UsingWaterfallsWithElip151);
        }
        let desc = descriptor.bitcoin_descriptor_without_key_origin();
        let desc = if self.waterfalls_avoid_encryption {
            desc
        } else {
            let recipient = self.waterfalls_server_recipient()?;

            // TODO ideally the encrypted descriptor should be cached and reused, so that caching can be leveraged
            encrypt(&desc, recipient)?
        };

        let response = self
            .client
            .get(descriptor_url)
            .query(&[("descriptor", desc)])
            .send()?;
        let status = response.status().as_u16();
        let body = response.text()?;

        if status != 200 {
            return Err(Error::Generic(body));
        }

        let waterfalls_result: WaterfallsResult = serde_json::from_str(&body)?;
        let mut data = Data::default();

        for (desc, chain_history) in waterfalls_result.txs_seen.iter() {
            let desc: elements_miniscript::Descriptor<DescriptorPublicKey> = desc.parse()?;
            let chain: Chain = (&desc)
                .try_into()
                .map_err(|_| Error::Generic("Cannot determine chain from desc".into()))?;
            let max = chain_history
                .iter()
                .enumerate()
                .filter(|(_, v)| !v.is_empty())
                .map(|(i, _)| i as u32)
                .max();
            if let Some(max) = max {
                data.last_unused[chain] = max + 1;
            }
            for (i, script_history) in chain_history.iter().enumerate() {
                // TODO handle paging by asking following pages if there are more than 1000 results
                let child = ChildNumber::from(waterfalls_result.page as u32 * 1000 + i as u32);
                let (script, cached) = state.get_or_derive(chain, child, &desc)?;
                if !cached {
                    data.scripts.insert(script, (chain, child));
                }
                for tx_seen in script_history {
                    let height = if tx_seen.height > 0 {
                        Some(tx_seen.height as u32)
                    } else {
                        None
                    };
                    if let Some(height) = height.as_ref() {
                        if let Some(block_hash) = tx_seen.block_hash.as_ref() {
                            data.height_blockhash.insert(*height, *block_hash);
                        }
                        if let Some(ts) = tx_seen.block_timestamp.as_ref() {
                            data.height_timestamp.insert(*height, *ts);
                        }
                    }

                    data.txid_height.insert(tx_seen.txid, height);
                }
            }
        }

        Ok(data)
    }
}

fn get_with_retry(url: &str, attempt: usize) -> Result<Response, Error> {
    let response = reqwest::blocking::get(url)?;
    tracing::debug!(
        "{} status_code:{} body bytes:{:?}",
        &url,
        response.status(),
        response.content_length(),
    );

    // 429 Too many requests
    // 503 Service Temporarily Unavailable
    if response.status() == 429 || response.status() == 503 {
        if attempt > 6 {
            return Err(Error::Generic("Too many retry".to_string()));
        }
        let secs = 1 << attempt;

        tracing::debug!("waiting {secs}");
        std::thread::sleep(std::time::Duration::from_secs(secs));
        get_with_retry(url, attempt + 1)
    } else {
        Ok(response)
    }
}

impl From<EsploraTx> for History {
    fn from(value: EsploraTx) -> Self {
        History {
            txid: value.txid,
            height: value.status.block_height.unwrap_or(-1),
            block_hash: value.status.block_hash,
            block_timestamp: None,
        }
    }
}

#[derive(Deserialize)]
struct EsploraTx {
    txid: elements::Txid,
    status: Status,
}

// TODO some of this fields may be Option in unconfirmed

#[derive(Deserialize)]
struct Status {
    block_height: Option<i32>,
    block_hash: Option<BlockHash>,
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::EsploraClient;
    use crate::BlockchainBackend;
    use elements::{encode::Decodable, BlockHash};

    fn get_block(base_url: &str, hash: BlockHash) -> elements::Block {
        let url = format!("{}/block/{}/raw", base_url, hash);
        let response = reqwest::blocking::get(url).unwrap();
        elements::Block::consensus_decode(&response.bytes().unwrap()[..]).unwrap()
    }

    #[ignore = "Should be integration test, but it is testing private function"]
    #[test]
    fn esplora_local() {
        let server = lwk_test_util::setup_with_esplora();

        let esplora_url = format!("http://{}", server.electrs.esplora_url.as_ref().unwrap());
        test_esplora_url(&esplora_url);
    }

    #[ignore]
    #[test]
    fn esplora_testnet() {
        test_esplora_url("https://blockstream.info/liquidtestnet/api");
        test_esplora_url("https://liquid.network/liquidtestnet/api");
    }

    fn test_esplora_url(esplora_url: &str) {
        println!("{}", esplora_url);

        let mut client = EsploraClient::new(esplora_url);
        let header = client.tip().unwrap();
        assert!(header.height > 100);

        let headers = client.get_headers(&[0], &HashMap::new()).unwrap();
        let genesis_header = &headers[0];
        assert_eq!(genesis_header.height, 0);

        let genesis_block = get_block(esplora_url, genesis_header.block_hash());
        let genesis_tx = &genesis_block.txdata[0];

        let txid = genesis_tx.txid();
        let txs = client.get_transactions(&[txid]).unwrap();

        assert_eq!(txs[0].txid(), txid);

        let existing_script = &genesis_tx.output[0].script_pubkey;

        let histories = client.get_scripts_history(&[existing_script]).unwrap();
        assert!(!histories.is_empty())
    }
}
