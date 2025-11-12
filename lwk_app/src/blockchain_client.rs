use lwk_wollet::clients::blocking::{BlockchainBackend, EsploraClient};
use lwk_wollet::clients::History;
use lwk_wollet::elements::{BlockHash, BlockHeader, Script, Transaction, Txid};
use lwk_wollet::{ElectrumClient, Error};
use std::collections::HashMap;

pub enum BlockchainClient {
    Electrum(ElectrumClient),
    Esplora(EsploraClient),
}

impl BlockchainBackend for BlockchainClient {
    fn tip(&mut self) -> Result<BlockHeader, Error> {
        match self {
            Self::Electrum(c) => c.tip(),
            Self::Esplora(c) => c.tip(),
        }
    }
    fn broadcast(&self, tx: &Transaction) -> Result<Txid, Error> {
        match self {
            Self::Electrum(c) => c.broadcast(tx),
            Self::Esplora(c) => c.broadcast(tx),
        }
    }
    fn get_transactions(&self, txids: &[Txid]) -> Result<Vec<Transaction>, Error> {
        match self {
            Self::Electrum(c) => c.get_transactions(txids),
            Self::Esplora(c) => c.get_transactions(txids),
        }
    }
    fn get_headers(
        &self,
        heights: &[u32],
        height_blockhash: &HashMap<u32, BlockHash>,
    ) -> Result<Vec<BlockHeader>, Error> {
        match self {
            Self::Electrum(c) => c.get_headers(heights, height_blockhash),
            Self::Esplora(c) => c.get_headers(heights, height_blockhash),
        }
    }
    fn get_scripts_history(&self, scripts: &[&Script]) -> Result<Vec<Vec<History>>, Error> {
        match self {
            Self::Electrum(c) => c.get_scripts_history(scripts),
            Self::Esplora(c) => c.get_scripts_history(scripts),
        }
    }
}
