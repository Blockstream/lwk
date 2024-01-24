use crate::store::Height;
use crate::{ElectrumUrl, Error};
use electrum_client::{Client, ElectrumApi, GetHistoryRes};
use elements::encode::deserialize as elements_deserialize;
use elements::encode::serialize as elements_serialize;
use elements::{bitcoin, BlockHash, BlockHeader, Script, Transaction, Txid};
use std::collections::HashMap;
use std::fmt::Debug;

use super::History;

pub struct ElectrumClient {
    client: Client,

    tip: BlockHeader,
}

impl Debug for ElectrumClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ElectrumClient")
            .field("tip", &self.tip)
            .finish()
    }
}

impl ElectrumClient {
    pub fn new(url: &ElectrumUrl) -> Result<Self, Error> {
        let client = url.build_client()?;
        let header = client.block_headers_subscribe_raw()?;
        let tip: BlockHeader = elements_deserialize(&header.header)?;

        Ok(Self { client, tip })
    }
}
impl super::BlockchainBackend for ElectrumClient {
    fn tip(&mut self) -> Result<BlockHeader, Error> {
        let mut popped_header = None;
        while let Some(header) = self.client.block_headers_pop_raw()? {
            popped_header = Some(header)
        }

        if let Some(popped_header) = popped_header {
            let tip: BlockHeader = elements_deserialize(&popped_header.header)?;
            self.tip = tip;
        }

        Ok(self.tip.clone())
    }

    fn broadcast(&self, tx: &Transaction) -> Result<Txid, Error> {
        let txid = self
            .client
            .transaction_broadcast_raw(&elements_serialize(tx))?;
        Ok(Txid::from_raw_hash(txid.to_raw_hash()))
    }

    fn get_transactions(&self, txids: &[Txid]) -> Result<Vec<Transaction>, Error> {
        let txids: Vec<bitcoin::Txid> = txids
            .iter()
            .map(|t| bitcoin::Txid::from_raw_hash(t.to_raw_hash()))
            .collect();

        let mut result = vec![];
        for tx in self.client.batch_transaction_get_raw(&txids)? {
            let tx: Transaction = elements::encode::deserialize(&tx)?;
            result.push(tx);
        }
        Ok(result)
    }

    fn get_headers(
        &self,
        heights: &[Height],
        _: &HashMap<Height, BlockHash>,
    ) -> Result<Vec<BlockHeader>, Error> {
        let mut result = vec![];
        for header in self.client.batch_block_header_raw(heights)? {
            let header: BlockHeader = elements::encode::deserialize(&header)?;
            result.push(header);
        }
        Ok(result)
    }

    fn get_scripts_history(&self, scripts: &[&Script]) -> Result<Vec<Vec<History>>, Error> {
        let scripts: Vec<&bitcoin::Script> = scripts
            .iter()
            .map(|t| bitcoin::Script::from_bytes(t.as_bytes()))
            .collect();

        Ok(self
            .client
            .batch_script_get_history(&scripts)?
            .into_iter()
            .map(|e| e.into_iter().map(Into::into).collect())
            .collect())
    }
}

impl From<GetHistoryRes> for History {
    fn from(value: GetHistoryRes) -> Self {
        History {
            txid: Txid::from_raw_hash(value.tx_hash.to_raw_hash()),
            height: value.height,
            block_hash: None,
        }
    }
}
