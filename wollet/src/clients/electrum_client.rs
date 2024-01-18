use crate::{ElectrumUrl, Error};
use electrum_client::{Client, ElectrumApi, GetHistoryRes};
use elements::encode::deserialize as elements_deserialize;
use elements::encode::serialize as elements_serialize;
use elements::{bitcoin, BlockHeader, Transaction, Txid};
use std::{borrow::Borrow, fmt::Debug};

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

    fn get_transactions<I>(&self, txids: I) -> Result<Vec<Transaction>, Error>
    where
        I: IntoIterator + Clone,
        I::Item: Borrow<elements::Txid>,
    {
        let txids: Vec<bitcoin::Txid> = txids
            .into_iter()
            .map(|t| bitcoin::Txid::from_raw_hash(t.borrow().to_raw_hash()))
            .collect();

        let mut result = vec![];
        for tx in self.client.batch_transaction_get_raw(&txids)? {
            let tx: Transaction = elements::encode::deserialize(&tx)?;
            result.push(tx);
        }
        Ok(result)
    }

    fn get_headers<I>(&self, heights: I) -> Result<Vec<BlockHeader>, Error>
    where
        I: IntoIterator + Clone,
        I::Item: Borrow<u32>,
    {
        let mut result = vec![];
        for header in self.client.batch_block_header_raw(heights)? {
            let header: BlockHeader = elements::encode::deserialize(&header)?;
            result.push(header);
        }
        Ok(result)
    }

    fn get_scripts_history<'s, I>(&self, scripts: I) -> Result<Vec<Vec<GetHistoryRes>>, Error>
    where
        I: IntoIterator + Clone,
        I::Item: Borrow<&'s elements::Script>,
    {
        let scripts: Vec<&bitcoin::Script> = scripts
            .into_iter()
            .map(|t| bitcoin::Script::from_bytes(t.borrow().as_bytes()))
            .collect();

        Ok(self.client.batch_script_get_history(&scripts)?)
    }
}
