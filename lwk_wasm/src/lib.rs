mod blockdata;
mod contract;
mod descriptor;
mod error;
mod esplora;
mod mnemonic;
mod network;
mod pset;
mod signer;
mod update;
mod wollet;

pub use blockdata::address::{Address, AddressResult};
pub use blockdata::asset_id::AssetId;
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
use lwk_jade::asyncr;
pub use mnemonic::Mnemonic;
pub use network::Network;
pub use pset::Pset;
pub use signer::Signer;
pub use update::Update;
pub use wollet::Wollet;

use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub async fn test_port() {
    web_sys::console::log_1(&"1".into());
    let window = web_sys::window().unwrap();
    let navigator = window.navigator();
    let serial = navigator.serial();
    web_sys::console::log_1(&"2".into());
    // web_sys::SerialPortRequestOptions::new
    let promise = serial.request_port();
    web_sys::console::log_1(&"3".into());

    let result = wasm_bindgen_futures::JsFuture::from(promise).await.unwrap();
    web_sys::console::log_1(&result);

    let serial: web_sys::SerialPort = result.dyn_into().unwrap();
    web_sys::console::log_1(&serial);

    let serial_options = web_sys::SerialOptions::new(115_200);
    web_sys::console::log_1(&serial_options);

    let promise = serial.open(&serial_options);
    web_sys::console::log_1(&"5".into());
    let result = wasm_bindgen_futures::JsFuture::from(promise).await.unwrap();

    web_sys::console::log_1(&serial);

    let jade = asyncr::Jade::new(&serial, lwk_jade::Network::TestnetLiquid);
    web_sys::console::log_1(&"6".into());
    let x = jade.version_info().await.unwrap();

    web_sys::console::log_1(&serde_wasm_bindgen::to_value(&x).unwrap());

    jade.unlock().await.unwrap();

    let xpub = jade.get_master_xpub().await.unwrap();
    web_sys::console::log_1(&xpub.to_string().into());

    let promise = serial.close();
    web_sys::console::log_1(&"closed".into());

    let result = wasm_bindgen_futures::JsFuture::from(promise).await;
    let e = match result {
        Ok(e) => e,
        Err(e) => e,
    };

    web_sys::console::log_1(&e);
}

#[cfg(test)]
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

    #[ignore = "require `RUSTFLAGS=--cfg=web_sys_unstable_apis`"]
    #[wasm_bindgen_test]
    async fn test_serial() {
        let window = web_sys::window().unwrap();
        let navigator = window.navigator();
        let serial = navigator.serial();
        let promise = serial.request_port();
        let result = wasm_bindgen_futures::JsFuture::from(promise).await.unwrap();
        panic!("{:?}", result)
    }
}
