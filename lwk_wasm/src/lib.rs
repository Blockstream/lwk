use lwk_wollet::{ElementsNetwork, EsploraWasmClient, NoPersist, Wollet, WolletDescriptor};
use std::{fmt::Debug, str::FromStr, sync::Arc};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub async fn balance(desc: &str) -> Result<JsValue, String> {
    let descriptor = WolletDescriptor::from_str(desc).map_err(to_debug)?;
    wasm_bindgen_test::console_log!("going to sync {}", descriptor);

    let network = ElementsNetwork::LiquidTestnet;
    let mut wollet = Wollet::new(network, Arc::new(NoPersist {}), descriptor).map_err(to_debug)?;

    let mut client = EsploraWasmClient::new("https://blockstream.info/liquidtestnet/api");

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

mod tests {
    use std::{collections::HashMap, str::FromStr};

    use lwk_wollet::elements::AssetId;
    use wasm_bindgen_test::*;

    wasm_bindgen_test_configure!(run_in_browser);

    #[wasm_bindgen_test]
    async fn balance_test() {
        let desc = "ct(slip77(0371e66dde8ab9f3cb19d2c20c8fa2d7bd1ddc73454e6b7ef15f0c5f624d4a86),elsh(wpkh([75ea4a43/49'/1'/0']tpubDDRMQzj8FGnDXxAhr8zgM22VT7BT2H2cPUdCRDSi3ima15TRUZEkT32zExr1feVReMYvBEm21drG1qKryjHf3cD6iD4j1nkPkbPDuQxCJG4/<0;1>/*)))#utnwh7dr";
        let balance = crate::balance(desc).await.unwrap();
        let balance: HashMap<AssetId, u64> = serde_wasm_bindgen::from_value(balance).unwrap();
        let mut expected = HashMap::new();
        expected.insert(
            AssetId::from_str("144c654344aa716d6f3abcc1ca90e5641e4e2a7f633bc09fe3baf64585819a49")
                .unwrap(),
            100000,
        );
        assert_eq!(
            expected, balance,
            "balance isn't as expected, it could be some coin has been received or spent"
        );
    }
}
