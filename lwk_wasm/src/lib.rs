use lwk_wollet::{ElementsNetwork, EsploraWasmClient, NoPersist};
use std::{fmt::Debug, str::FromStr, sync::Arc};
use wasm_bindgen::prelude::*;

mod blockdata;
mod descriptor;
mod error;
mod esplora;
mod network;
mod pset;
mod update;
mod wollet;

pub use blockdata::address::{Address, AddressResult};
pub use blockdata::asset_id::AssetId;
pub use blockdata::out_point::OutPoint;
pub use blockdata::script::Script;
pub use blockdata::transaction::{Transaction, Txid};
pub use blockdata::tx_out_secrets::TxOutSecrets;
pub use descriptor::WolletDescriptor;
pub(crate) use error::Error;
pub use esplora::EsploraClient;
pub use network::Network;
pub use pset::Pset;
pub use update::Update;
pub use wollet::Wollet;

/// Calculate the balance of the given descriptor
///
/// if the descriptor contains "xpub" will be checked liquid mainnet, otherwise liquid testnet.
#[wasm_bindgen]
pub async fn balance(desc: &str) -> Result<JsValue, String> {
    let descriptor = lwk_wollet::WolletDescriptor::from_str(desc).map_err(to_debug)?;
    wasm_bindgen_test::console_log!("going to sync {}", descriptor);

    let network = if desc.contains("xpub") {
        ElementsNetwork::Liquid
    } else {
        ElementsNetwork::LiquidTestnet
    };

    let mut wollet =
        lwk_wollet::Wollet::new(network, Arc::new(NoPersist {}), descriptor).map_err(to_debug)?;

    let url = match network {
        ElementsNetwork::Liquid => "https://blockstream.info/liquid/api",
        _ => "https://blockstream.info/liquidtestnet/api",
    };

    let mut client = EsploraWasmClient::new(url);

    let update = client.full_scan(&wollet).await.map_err(to_debug)?;
    if let Some(update) = update {
        wollet.apply_update(update).map_err(to_debug)?;
    }
    let balance = wollet.balance().map_err(to_debug)?;
    wasm_bindgen_test::console_log!("balance {:?}", balance);

    serde_wasm_bindgen::to_value(&balance).map_err(to_debug)
}

fn to_debug<D: Debug>(d: D) -> String {
    format!("{d:?}")
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use lwk_wollet::{elements::AssetId, ElementsNetwork};
    use wasm_bindgen_test::*;

    wasm_bindgen_test_configure!(run_in_browser);

    #[wasm_bindgen_test]
    async fn balance_test_mainnet() {
        let desc = "ct(slip77(0371e66dde8ab9f3cb19d2c20c8fa2d7bd1ddc73454e6b7ef15f0c5f624d4a86),elsh(wpkh([75ea4a43/49'/1776'/0']xpub6D3Y5EKNsmegjE7azkF2foAYFivHrV5u7tcnN2TXELxv1djNtabCHtp3jMvxqEhTU737mYSUqHD1sA5MdZXQ8DWJLNft1gwtpzXZDsRnrZd/<0;1>/*)))#efvhq75f";
        balance_test(desc, ElementsNetwork::Liquid, 5000).await;
    }

    #[wasm_bindgen_test]
    async fn balance_test_testnet() {
        let desc = "ct(slip77(0371e66dde8ab9f3cb19d2c20c8fa2d7bd1ddc73454e6b7ef15f0c5f624d4a86),elsh(wpkh([75ea4a43/49'/1'/0']tpubDDRMQzj8FGnDXxAhr8zgM22VT7BT2H2cPUdCRDSi3ima15TRUZEkT32zExr1feVReMYvBEm21drG1qKryjHf3cD6iD4j1nkPkbPDuQxCJG4/<0;1>/*)))#utnwh7dr";
        balance_test(desc, ElementsNetwork::LiquidTestnet, 100000).await;
    }

    #[wasm_bindgen_test]
    async fn balance_err() {
        let err = crate::balance("").await.unwrap_err();
        let expected = "ElementsMiniscript(BadDescriptor(\"Not a CT Descriptor\"))";
        assert_eq!(err, expected);

        let desc_no_checksum = "ct(slip77(0371e66dde8ab9f3cb19d2c20c8fa2d7bd1ddc73454e6b7ef15f0c5f624d4a86),elsh(wpkh([75ea4a43/49'/1'/0']tpubDDRMQzj8FGnDXxAhr8zgM22VT7BT2H2cPUdCRDSi3ima15TRUZEkT32zExr1feVReMYvBEm21drG1qKryjHf3cD6iD4j1nkPkbPDuQxCJG4/<0;1>/*)))#";
        let err = crate::balance(desc_no_checksum).await.unwrap_err();
        let expected =
            "ElementsMiniscript(BadDescriptor(\"Invalid checksum '', expected 'utnwh7dr'\"))";
        assert_eq!(err, expected);
    }

    async fn balance_test(desc: &str, network: ElementsNetwork, expected_at_least: u64) {
        let balance = crate::balance(desc).await.unwrap();
        let balance: HashMap<AssetId, u64> = serde_wasm_bindgen::from_value(balance).unwrap();
        assert!(
            *balance.get(&network.policy_asset()).unwrap() >= expected_at_least,
            "balance isn't as expected, it could be some coin has been spent"
        )
    }
}
