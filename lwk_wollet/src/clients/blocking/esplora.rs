use age::x25519::Recipient;
use elements::{BlockHash, Script, Txid};
use std::collections::{HashMap, HashSet};
use tokio::runtime::Runtime;

use crate::{
    clients::{asyncr, Capability, Data, EsploraClientBuilder, History},
    store::Height,
    wollet::WolletState,
    ElementsNetwork, Error, WolletDescriptor,
};

use super::BlockchainBackend;

impl EsploraClientBuilder {
    /// Build a blocking Esplora client
    pub fn build_blocking(self) -> Result<EsploraClient, Error> {
        Ok(EsploraClient {
            rt: Runtime::new()?,
            client: EsploraClientBuilder::build(self)?,
        })
    }
}

#[derive(Debug)]
/// A blockchain backend implementation based on the
/// [esplora HTTP API](https://github.com/blockstream/esplora/blob/master/API.md)
/// But can also use the [waterfalls](https://github.com/RCasatta/waterfalls) endpoint to speed up the scan if supported by the server.
pub struct EsploraClient {
    rt: Runtime,
    client: asyncr::EsploraClient,
}

impl EsploraClient {
    /// Create a new Esplora client
    pub fn new(url: &str, network: ElementsNetwork) -> Result<Self, Error> {
        Ok(Self {
            rt: Runtime::new()?,
            client: asyncr::EsploraClient::new(network, url),
        })
    }
}

/// "Waterfalls" methods
impl EsploraClient {
    /// Create a new Esplora client using the "waterfalls" endpoint
    pub fn new_waterfalls(url: &str, network: ElementsNetwork) -> Result<Self, Error> {
        Ok(Self {
            rt: Runtime::new()?,
            client: EsploraClientBuilder::new(url, network)
                .waterfalls(true)
                .build()?,
        })
    }

    /// Do not encrypt the descriptor when using the "waterfalls" endpoint
    pub fn waterfalls_avoid_encryption(&mut self) {
        self.client.waterfalls_avoid_encryption = true;
    }

    /// Returns the waterfall server recipient key using a cached value or by asking the server its key
    pub fn waterfalls_server_recipient(&mut self) -> Result<Recipient, Error> {
        self.rt.block_on(self.client.waterfalls_server_recipient())
    }

    /// Set the waterfalls server recipient key. This is used to encrypt the descriptor when calling the waterfalls endpoint.
    pub fn set_waterfalls_server_recipient(&mut self, recipient: Recipient) {
        self.client.set_waterfalls_server_recipient(recipient);
    }
}

impl BlockchainBackend for EsploraClient {
    fn tip(&mut self) -> Result<elements::BlockHeader, crate::Error> {
        self.rt.block_on(self.client.tip())
    }

    fn broadcast(&self, tx: &elements::Transaction) -> Result<elements::Txid, crate::Error> {
        self.rt.block_on(self.client.broadcast(tx))
    }

    fn get_transactions(&self, txids: &[Txid]) -> Result<Vec<elements::Transaction>, Error> {
        self.rt.block_on(self.client.get_transactions(txids))
    }

    fn get_headers(
        &self,
        heights: &[Height],
        height_blockhash: &HashMap<Height, BlockHash>,
    ) -> Result<Vec<elements::BlockHeader>, Error> {
        self.rt
            .block_on(self.client.get_headers(heights, height_blockhash))
    }

    // examples:
    // https://blockstream.info/liquidtestnet/api/address/tex1qntw9m0j2e93n84x975t47ddhgkzx3x8lhfv2nj/txs
    // https://blockstream.info/liquidtestnet/api/scripthash/b50a2a798d876db54acfa0d8dfdc49154ea8defed37b225ec4c9ec7415358ba3/txs
    fn get_scripts_history(&self, scripts: &[&Script]) -> Result<Vec<Vec<History>>, Error> {
        self.rt.block_on(self.client.get_scripts_history(scripts))
    }

    fn capabilities(&self) -> HashSet<Capability> {
        self.client.capabilities()
    }

    fn get_history_waterfalls<S: WolletState>(
        &mut self,
        descriptor: &WolletDescriptor,
        state: &S,
        to_index: u32,
    ) -> Result<Data, Error> {
        self.rt.block_on(
            self.client
                .get_history_waterfalls(descriptor, state, to_index),
        )
    }

    fn utxo_only(&self) -> bool {
        self.client.utxo_only
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::EsploraClient;
    use crate::{clients::blocking::BlockchainBackend, ElementsNetwork};
    use elements::{encode::Decodable, BlockHash};

    fn get_block(base_url: &str, hash: BlockHash) -> elements::Block {
        let url = format!("{base_url}/block/{hash}/raw");
        let response = reqwest::blocking::get(url).unwrap();
        elements::Block::consensus_decode(&response.bytes().unwrap()[..]).unwrap()
    }

    #[ignore = "Should be integration test, but it is testing private function"]
    #[test]
    fn esplora_local() {
        let env = lwk_test_util::TestEnvBuilder::from_env()
            .with_esplora()
            .build();

        test_esplora_url(&env.esplora_url());
    }

    #[ignore]
    #[test]
    fn esplora_testnet() {
        test_esplora_url("https://blockstream.info/liquidtestnet/api");
        test_esplora_url("https://liquid.network/liquidtestnet/api");
    }

    fn test_esplora_url(esplora_url: &str) {
        println!("{esplora_url}");

        let mut client =
            EsploraClient::new(esplora_url, ElementsNetwork::default_regtest()).unwrap();
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

        // Test get_transaction method
        let _tx = txs[0].clone();

        // Test get_transactions method with the same txid
        let txs_batch = client.get_transactions(&[txid]).unwrap();
        assert_eq!(txs_batch.len(), 1);
        assert_eq!(txs_batch[0].txid(), txid);

        // Test get_transactions with multiple txids if there are more transactions in the block
        if genesis_block.txdata.len() > 1 {
            let txid2 = genesis_block.txdata[1].txid();
            let txids = vec![txid, txid2];
            let txs_multi = client.get_transactions(&txids).unwrap();
            assert_eq!(txs_multi.len(), 2);
            assert_eq!(txs_multi[0].txid(), txid);
            assert_eq!(txs_multi[1].txid(), txid2);
        }
    }
}
