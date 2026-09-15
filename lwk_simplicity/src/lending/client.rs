use lwk_wollet::{
    blocking::{BlockchainBackend, EsploraClient},
    elements::{Transaction, Txid},
    ElectrumClient, Wollet,
};

pub enum AnyClient {
    Electrum(Box<ElectrumClient>),
    Esplora(EsploraClient),
}

impl AnyClient {
    #[allow(dead_code)]
    pub fn broadcast(&self, tx: &Transaction) -> Result<Txid, lwk_wollet::Error> {
        match self {
            AnyClient::Electrum(c) => c.broadcast(tx),
            AnyClient::Esplora(c) => c.broadcast(tx),
        }
    }

    pub fn full_scan(
        &mut self,
        wollet: &Wollet,
    ) -> Result<Option<lwk_wollet::Update>, lwk_wollet::Error> {
        match self {
            AnyClient::Electrum(c) => c.full_scan(wollet),
            AnyClient::Esplora(c) => c.full_scan(wollet),
        }
    }

    pub fn get_transaction(&self, txid: Txid) -> Result<Transaction, lwk_wollet::Error> {
        match self {
            AnyClient::Electrum(c) => c.get_transaction(txid),
            AnyClient::Esplora(c) => c.get_transaction(txid),
        }
    }

    pub fn tip(&mut self) -> Result<lwk_wollet::elements::BlockHeader, lwk_wollet::Error> {
        match self {
            AnyClient::Electrum(c) => c.tip(),
            AnyClient::Esplora(c) => c.tip(),
        }
    }
}
