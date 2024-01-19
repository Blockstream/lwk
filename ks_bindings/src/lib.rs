pub mod blockdata;
mod chain;
mod desc;
mod electrum_client;
mod error;
mod mnemonic;
mod network;
mod pset;
mod signer;
pub mod types;
mod update;
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
pub use electrum_client::ElectrumClient;
pub use mnemonic::Mnemonic;
pub use network::Network;
pub use pset::Pset;
pub use signer::Signer;
pub use update::Update;
pub use wollet::Wollet;

pub use error::Error;
uniffi::setup_scaffolding!();

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use crate::{wollet::Wollet, Address, ElectrumClient, Mnemonic, Network, Signer, Txid};

    #[test]
    fn test_ks_flow() {
        let datadir = "/tmp/.ks";
        let mnemonic = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
        let network: Network = test_util::network_regtest().into();
        let signer = Signer::new(&Mnemonic::new(mnemonic.to_string()).unwrap(), &network).unwrap();

        let server = test_util::setup(false);

        let singlesig_desc = signer.wpkh_slip77_descriptor().unwrap();

        let electrum_client =
            ElectrumClient::new(server.electrs.electrum_url.to_string(), false, false).unwrap();

        let wollet = Wollet::new(&network, &singlesig_desc, datadir.to_string()).unwrap();
        let _latest_address = wollet.address(None); // lastUnused
        let address_0 = wollet.address(Some(0)).unwrap();
        let expected_address_0 = "el1qq2xvpcvfup5j8zscjq05u2wxxjcyewk7979f3mmz5l7uw5pqmx6xf5xy50hsn6vhkm5euwt72x878eq6zxx2z0z676mna6kdq";
        assert_eq!(expected_address_0, address_0.address().to_string());

        let txid = server.node_sendtoaddress(
            &elements::Address::from_str(expected_address_0).unwrap(),
            100000000,
            None,
        );
        wollet.wait_for_tx(Txid::from_str(&txid).unwrap(), &electrum_client);

        let address_1 = wollet.address(Some(1)).unwrap();
        let expected_address_1 = "el1qqv8pmjjq942l6cjq69ygtt6gvmdmhesqmzazmwfsq7zwvan4kewdqmaqzegq50r2wdltkfsw9hw20zafydz4sqljz0eqe0vhc";
        assert_eq!(expected_address_1, address_1.address().to_string());

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

        let out_address = Address::new(expected_address_1.to_string()).unwrap();
        let satoshis = 900;
        let fee_rate = 280_f32; // this seems like absolute fees
        let pset = wollet
            .create_lbtc_tx(&out_address, satoshis, fee_rate)
            .unwrap();
        let signed_pset = signer.sign(&pset).unwrap();
        let finalized_pset = wollet.finalize(&signed_pset).unwrap();
        let txid = electrum_client
            .broadcast(&finalized_pset.extract_tx().unwrap())
            .unwrap();
        println!("BROADCASTED TX!\nTXID: {:?}", txid);
    }
}
