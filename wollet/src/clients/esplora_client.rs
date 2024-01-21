use std::{collections::HashMap, str::FromStr};

use elements::{
    encode::Decodable,
    hashes::{hex::FromHex, sha256, Hash},
    hex::ToHex,
    pset::serialize::Serialize,
    BlockHash, Script, Txid,
};
use serde::Deserialize;

use crate::{store::Height, BlockchainBackend, Error};

use super::History;

/// A blockchain backend implementation based on the
/// [esplora HTTP API](https://github.com/blockstream/esplora/blob/master/API.md)
pub struct EsploraClient {
    base_url: String,
    tip_hash_url: String,
    broadcast_url: String,
}

impl EsploraClient {
    pub fn new(url: &str) -> Self {
        Self {
            base_url: url.to_string(),
            tip_hash_url: format!("{url}/blocks/tip/hash"),
            broadcast_url: format!("{url}/tx"),
        }
    }

    fn last_block_hash(&mut self) -> Result<elements::BlockHash, crate::Error> {
        let response = minreq::get(&self.tip_hash_url).send()?;
        Ok(BlockHash::from_str(response.as_str()?)?)
    }
}

impl BlockchainBackend for EsploraClient {
    fn tip(&mut self) -> Result<elements::BlockHeader, crate::Error> {
        let last_block_hash = self.last_block_hash()?;

        let header_url = format!("{}/block/{}/header", self.base_url, last_block_hash);
        let response = minreq::get(header_url).send()?;
        let header_bytes = Vec::<u8>::from_hex(response.as_str()?)?;

        let header = elements::BlockHeader::consensus_decode(&header_bytes[..])?;
        Ok(header)
    }

    fn broadcast(&self, tx: &elements::Transaction) -> Result<elements::Txid, crate::Error> {
        let tx_bytes = tx.serialize();
        let response = minreq::post(&self.broadcast_url)
            .with_body(tx_bytes)
            .send()?;
        let txid = elements::Txid::from_str(response.as_str()?)?;
        Ok(txid)
    }

    fn get_transactions(&self, txids: &[Txid]) -> Result<Vec<elements::Transaction>, Error> {
        // TODO make parallel requests
        let mut result = vec![];
        for txid in txids.into_iter() {
            let tx_url = format!("{}/tx/{}/raw", self.base_url, txid);
            let response = minreq::get(tx_url).send()?;
            let tx = elements::Transaction::consensus_decode(response.as_bytes())?;
            result.push(tx);
        }
        Ok(result)
    }

    fn get_headers(
        &self,
        heights: &[Height],
        height_blockhash: &HashMap<Height, BlockHash>,
    ) -> Result<Vec<elements::BlockHeader>, Error> {
        // TODO make parallel requests
        let mut result = vec![];
        for height in heights.into_iter() {
            let block_hash = match height_blockhash.get(height) {
                Some(block_hash) => *block_hash,
                None => {
                    let block_height = format!("{}/block-height/{}", self.base_url, height);
                    let response = minreq::get(block_height).send()?;
                    BlockHash::from_str(response.as_str()?)?
                }
            };

            let block_header = format!("{}/block/{}/header", self.base_url, block_hash);
            let response = minreq::get(block_header).send()?;
            let header_bytes = Vec::<u8>::from_hex(response.as_str()?)?;

            let header = elements::BlockHeader::consensus_decode(&header_bytes[..])?;

            result.push(header);
        }
        Ok(result)
    }

    // examples:
    // https://blockstream.info/liquidtestnet/api/address/tex1qntw9m0j2e93n84x975t47ddhgkzx3x8lhfv2nj/txs
    // https://blockstream.info/liquidtestnet/api/scripthash/b50a2a798d876db54acfa0d8dfdc49154ea8defed37b225ec4c9ec7415358ba3/txs
    fn get_scripts_history(&self, scripts: &[&Script]) -> Result<Vec<Vec<History>>, Error> {
        // TODO make parallel requests
        let mut result: Vec<_> = vec![];
        for script in scripts.into_iter() {
            let script = elements::bitcoin::Script::from_bytes(script.as_bytes());
            let script_hash = sha256::Hash::hash(script.as_bytes()).to_byte_array();
            let url = format!("{}/scripthash/{}/txs", self.base_url, script_hash.to_hex());
            // TODO must handle paging -> https://github.com/blockstream/esplora/blob/master/API.md#addresses
            let response = minreq::get(url).send()?;
            let json: Vec<EsploraTx> = response.json()?;

            let history: Vec<History> = json.into_iter().map(Into::into).collect();
            result.push(history)
        }
        Ok(result)
    }
}

impl From<EsploraTx> for History {
    fn from(value: EsploraTx) -> Self {
        History {
            txid: value.txid,
            height: value.status.block_height,
            block_hash: Some(value.status.block_hash),
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
    block_height: i32,
    block_hash: BlockHash,
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::EsploraClient;
    use crate::BlockchainBackend;
    use elements::{encode::Decodable, BlockHash};

    fn get_block(base_url: &str, hash: BlockHash) -> elements::Block {
        let url = format!("{}/block/{}/raw", base_url, hash);
        let response = minreq::get(url).send().unwrap();
        let block = elements::Block::consensus_decode(response.as_bytes()).unwrap();
        block
    }

    #[test]
    fn esplora_local() {
        let server = test_util::setup(true);

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

        let mut client = EsploraClient::new(&esplora_url);
        let header = client.tip().unwrap();
        assert!(header.height > 100);

        let headers = client.get_headers(&vec![0], &HashMap::new()).unwrap();
        let genesis_header = &headers[0];
        assert_eq!(genesis_header.height, 0);

        let genesis_block = get_block(&esplora_url, genesis_header.block_hash());
        let genesis_tx = &genesis_block.txdata[0];

        let txid = genesis_tx.txid();
        let txs = client.get_transactions(&vec![txid]).unwrap();

        assert_eq!(txs[0].txid(), txid);

        let existing_script = &genesis_tx.output[0].script_pubkey;

        let histories = client.get_scripts_history(&vec![existing_script]).unwrap();
        assert!(!histories.is_empty())
    }
}
