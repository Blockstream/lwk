pub mod blockdata;
mod chain;
mod desc;
mod error;
mod mnemonic;
mod network;
mod pset;
mod signer;
pub mod types;
mod wollet;

pub use blockdata::address::Address;
pub use blockdata::address_result::AddressResult;
pub use blockdata::out_point::OutPoint;
pub use blockdata::script::Script;
pub use blockdata::transaction::Transaction;
pub use blockdata::tx_out_secrets::TxOutSecrets;
pub use blockdata::txid::Txid;
pub use blockdata::wallet_tx::WalletTx;
pub use blockdata::wallet_tx_out::WalletTxOut;

pub use chain::Chain;
pub use desc::WolletDescriptor;
pub use mnemonic::Mnemonic;
pub use network::ElementsNetwork;
pub use pset::Pset;
pub use signer::Signer;
pub use wollet::Wollet;

pub use error::Error;
uniffi::setup_scaffolding!();

#[cfg(test)]
mod tests {
    use crate::{network::ElementsNetwork, wollet::Wollet, Address, Mnemonic, Signer};

    #[test]
    fn test_ks_flow() {
        let datadir = "/tmp/.ks";
        let mnemonic = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
        let signer = Signer::new(&Mnemonic::new(mnemonic.to_string()).unwrap()).unwrap();

        let singlesig_desc = signer.wpkh_slip77_descriptor().unwrap();
        let wollet = Wollet::new(
            ElementsNetwork::LiquidTestnet,
            &singlesig_desc,
            datadir.to_string(),
        )
        .unwrap();
        let _latest_address = wollet.address(None); // lastUnused
        let address_0 = wollet.address(Some(0)).unwrap();
        let expected_address_0 = "tlq1qq2xvpcvfup5j8zscjq05u2wxxjcyewk7979f3mmz5l7uw5pqmx6xf5xy50hsn6vhkm5euwt72x878eq6zxx2z58hd7zrsg9qn";
        assert_eq!(expected_address_0, address_0.address().to_string());
        let _ = wollet.sync();
        let balance = wollet.balance();
        println!("{:?}", balance);
        let txs = wollet.transactions().unwrap();
        for tx in txs {
            for output in tx.outputs() {
                let script_pubkey = match output.as_ref() {
                    Some(out) => out.script_pubkey().to_string(),
                    None => "Not a spendable scriptpubkey".to_string(),
                };
                let value = match output.as_ref() {
                    Some(out) => out.unblinded().value(),
                    None => 0,
                };
                println!("script_pubkey: {:?}, value: {}", script_pubkey, value)
            }
        }

        let out_address = Address::new( "tlq1qq0l36r57ys6nnz3xdp0eeunyuuh9dvq2fvyzj58aqaavqksenejj7plcd8mp7d9g6rxuctnj5q4cjxlu6h4tkqzv92w860z5x".to_string()).unwrap();
        let satoshis = 900;
        let fee_rate = 280_f32; // this seems like absolute fees
        let pset = wollet
            .create_lbtc_tx(&out_address, satoshis, fee_rate)
            .unwrap();
        let signed_pset = signer.sign(&pset).unwrap();
        let txid = wollet.broadcast(&signed_pset).unwrap();
        println!("BROADCASTED TX!\nTXID: {:?}", txid);
    }
}
