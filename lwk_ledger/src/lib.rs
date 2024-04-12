use ledger_bitcoin_client::client::BitcoinClient;

mod transport;
use transport::TransportTcp;

pub fn new(port: u16) -> BitcoinClient<TransportTcp> {
    BitcoinClient::new(TransportTcp::new(port).expect("TODO"))
}
