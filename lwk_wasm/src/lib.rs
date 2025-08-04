#![cfg_attr(docsrs, feature(doc_auto_cfg))]
#![doc = include_str!("../README.md")]

mod amp2;
mod bip;
mod blockdata;
mod contract;
mod descriptor;
mod error;
mod esplora;
#[cfg(all(feature = "serial", target_arch = "wasm32"))]
mod jade;
#[cfg(all(feature = "serial", target_arch = "wasm32"))]
mod jade_websocket;
#[cfg(all(feature = "serial", target_arch = "wasm32"))]
mod ledger;
mod liquidex;
mod mnemonic;
mod network;
mod precision;
mod pset;
mod pset_details;
mod registry;
#[cfg(all(feature = "serial", target_arch = "wasm32"))]
mod serial;
mod signer;
mod tx_builder;
mod update;
#[cfg(all(feature = "serial", target_arch = "wasm32"))]
mod websocket;
mod wollet;
mod xpub;

pub use amp2::{Amp2, Amp2Descriptor};
pub use bip::Bip;
pub use blockdata::address::{Address, AddressResult};
pub use blockdata::asset_id::{AssetId, AssetIds};
pub use blockdata::out_point::OutPoint;
pub use blockdata::script::Script;
pub use blockdata::transaction::{Transaction, Txid};
pub use blockdata::tx_out_secrets::TxOutSecrets;
pub use blockdata::wallet_tx::WalletTx;
pub use blockdata::wallet_tx_out::{OptionWalletTxOut, WalletTxOut};
pub use contract::Contract;
pub use descriptor::WolletDescriptor;
pub(crate) use error::Error;
pub use esplora::EsploraClient;
#[cfg(all(feature = "serial", target_arch = "wasm32"))]
pub use jade::{Jade, Singlesig};
#[cfg(all(feature = "serial", target_arch = "wasm32"))]
pub use jade_websocket::JadeWebSocket;
pub use mnemonic::Mnemonic;
pub use network::Network;
pub use precision::Precision;
pub use pset::Pset;
pub use pset_details::{Issuance, PsetDetails};
pub use registry::{AssetMeta, Registry, RegistryPost};
pub use signer::Signer;
pub use tx_builder::TxBuilder;
pub use update::Update;
pub use wollet::Wollet;
pub use xpub::Xpub;

#[cfg(all(feature = "serial", target_arch = "wasm32"))]
pub use ledger::search_ledger_device;

#[cfg(all(test, target_arch = "wasm32"))]
mod tests {
    use std::collections::HashMap;

    use lwk_wollet::elements::AssetId;
    use wasm_bindgen_test::*;

    use crate::{Network, Wollet, WolletDescriptor};

    wasm_bindgen_test_configure!(run_in_browser);

    #[ignore = "require network calls"]
    #[wasm_bindgen_test]
    async fn balance_test_mainnet() {
        let desc = "ct(slip77(0371e66dde8ab9f3cb19d2c20c8fa2d7bd1ddc73454e6b7ef15f0c5f624d4a86),elsh(wpkh([75ea4a43/49'/1776'/0']xpub6D3Y5EKNsmegjE7azkF2foAYFivHrV5u7tcnN2TXELxv1djNtabCHtp3jMvxqEhTU737mYSUqHD1sA5MdZXQ8DWJLNft1gwtpzXZDsRnrZd/<0;1>/*)))#efvhq75f";
        balance_test(desc, Network::mainnet(), 5000).await;
    }

    #[ignore = "require network calls"]
    #[wasm_bindgen_test]
    async fn balance_test_testnet() {
        let desc = "ct(slip77(0371e66dde8ab9f3cb19d2c20c8fa2d7bd1ddc73454e6b7ef15f0c5f624d4a86),elsh(wpkh([75ea4a43/49'/1'/0']tpubDDRMQzj8FGnDXxAhr8zgM22VT7BT2H2cPUdCRDSi3ima15TRUZEkT32zExr1feVReMYvBEm21drG1qKryjHf3cD6iD4j1nkPkbPDuQxCJG4/<0;1>/*)))#utnwh7dr";
        balance_test(desc, Network::testnet(), 100000).await;
    }

    async fn balance_test(desc: &str, network: Network, expected_at_least: u64) {
        let descriptor = WolletDescriptor::new(desc).unwrap();
        let mut client = network.default_esplora_client();
        let mut wollet = Wollet::new(&network, &descriptor).unwrap();
        let update = client.full_scan(&wollet).await.unwrap().unwrap();
        wollet.apply_update(&update).unwrap();
        let balance = wollet.balance().unwrap();
        let balance: HashMap<AssetId, u64> = serde_wasm_bindgen::from_value(balance).unwrap();
        assert!(
            *balance.get(&(network.policy_asset().into())).unwrap() >= expected_at_least,
            "balance isn't as expected, it could be some coin has been spent"
        )
    }

    #[wasm_bindgen_test]
    async fn test_data() {
        let network = Network::testnet();
        let mut client = network.default_esplora_client();

        let mnemonic = crate::Mnemonic::new(include_str!(
            "../test_data/update_with_mnemonic/mnemonic.txt"
        ))
        .unwrap();
        let signer = crate::Signer::new(&mnemonic, &network).unwrap();
        let descriptor = signer.wpkh_slip77_descriptor().unwrap();
        let expected = include_str!("../test_data/update_with_mnemonic/descriptor.txt");
        assert_eq!(format!("{}", descriptor), expected);
        let mut wollet = Wollet::new(&Network::testnet(), &descriptor).unwrap();
        let address = wollet.address(None).unwrap().address();
        let expected = "tlq1qqwql6y6tswwhdx5423yraz27fghllv04tutsgwje6sumc34ux8pmpv2n9ruj4sy23my2yvwz5cknhlcacjkavu07vn5fr5e8s";
        assert_eq!(address.to_string(), expected);

        let update_base64 =
            include_str!("../test_data/update_with_mnemonic/update_serialized_encrypted.txt");
        let update =
            crate::Update::deserialize_decrypted_base64(update_base64, &descriptor).unwrap();
        wollet.apply_update(&update).unwrap();
        let utxos = wollet.utxos().unwrap();
        assert_eq!(utxos.len(), 1);
    }

    #[wasm_bindgen_test]
    async fn test_data2() {
        let network = Network::testnet();
        let mnemonic = crate::Mnemonic::new(include_str!(
            "../test_data/update_with_mnemonic/mnemonic2.txt"
        ))
        .unwrap();
        let signer = crate::Signer::new(&mnemonic, &network).unwrap();
        let descriptor = signer.wpkh_slip77_descriptor().unwrap();
        let expected = include_str!("../test_data/update_with_mnemonic/descriptor2.txt");
        assert_eq!(format!("{}", descriptor), expected);
        let mut wollet = Wollet::new(&Network::testnet(), &descriptor).unwrap();
        let address = wollet.address(None).unwrap().address();
        let expected = "tlq1qqge8nc4myrnfhczje9axcu8agchucgllvcnrvc5ezufqt9guq00vwer0jdryetd8z9dkqjh25yr50vun7qd0yc6g6nv63n0ak";
        assert_eq!(address.to_string(), expected);

        let update_base64 =
            include_str!("../test_data/update_with_mnemonic/update_serialized_encrypted2.txt");
        let update =
            crate::Update::deserialize_decrypted_base64(update_base64, &descriptor).unwrap();
        wollet.apply_update(&update).unwrap();
        let utxos = wollet.utxos().unwrap();
        assert_eq!(utxos.len(), 2); // 2 utxos, one for the tLBTC and the other for the asset 38fca2d939696061a8f76d4e6b5eecd54e3b4221c846f24a6b279e79952850a5
    }
}
