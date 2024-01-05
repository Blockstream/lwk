mod desc;
mod error;
mod network;
pub mod pset;
pub mod transaction;
pub mod types;
mod wallet_tx;
mod wollet;

pub use error::Error;
uniffi::setup_scaffolding!();

#[cfg(test)]
mod tests {
    use crate::{desc::singlesig_desc_from_mnemonic, network::ElementsNetwork, wollet::Wollet};

    #[test]
    fn test_ks_flow() {
        let datadir = "/tmp/.ks";
        let mnemonic = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
        let singlesig_desc = singlesig_desc_from_mnemonic(mnemonic.to_string()).unwrap();
        let wollet = Wollet::new(
            ElementsNetwork::LiquidTestnet,
            singlesig_desc.clone(),
            datadir.to_string(),
        )
        .unwrap();
        let _latest_address = wollet.address(None); // lastUnused
        let address_0 = wollet.address(Some(0)).unwrap();
        let expected_address_0 = "tlq1qq2xvpcvfup5j8zscjq05u2wxxjcyewk7979f3mmz5l7uw5pqmx6xf5xy50hsn6vhkm5euwt72x878eq6zxx2z58hd7zrsg9qn";
        assert_eq!(expected_address_0, address_0);
        let _ = wollet.sync();
        let balance = wollet.balance();
        println!("{:?}", balance);
        let txs = wollet.transactions().unwrap();
        for tx in txs {
            // NOTE: accessing inner in destination binding language is not possible, must be done
            // via mapping it to string which returns a json object.
            let tx = &tx.inner;

            for output in &tx.outputs {
                let script_pubkey = match output.as_ref() {
                    Some(out) => out.script_pubkey.to_string(),
                    None => "Not a spendable scriptpubkey".to_string(),
                };
                let value = match output.as_ref() {
                    Some(out) => out.unblinded.value,
                    None => 0,
                };
                println!("script_pubkey: {:?}, value: {}", script_pubkey, value)
            }
        }

        let out_address = "tlq1qq0l36r57ys6nnz3xdp0eeunyuuh9dvq2fvyzj58aqaavqksenejj7plcd8mp7d9g6rxuctnj5q4cjxlu6h4tkqzv92w860z5x";
        let satoshis = 900;
        let fee_rate = 280_f32; // this seems like absolute fees
        let pset_string = wollet
            .create_lbtc_tx(out_address.to_string(), satoshis, fee_rate)
            .unwrap();
        let signed_hex = wollet.sign_tx(mnemonic.to_string(), pset_string).unwrap();
        let txid = wollet.broadcast(signed_hex.parse().unwrap()).unwrap();
        println!("BROADCASTED TX!\nTXID: {:?}", txid);
    }
}
