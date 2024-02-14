mod utils;

use std::{str::FromStr, sync::Arc};

use lwk_wollet::{
    BlockchainBackend, ElementsNetwork, EsploraClient, NoPersist, Wollet, WolletDescriptor,
};
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::spawn_local;

#[wasm_bindgen]
pub async fn greet() {
    let descriptor = WolletDescriptor::from_str("ct(slip77(ab5824f4477b4ebb00a132adfd8eb0b7935cf24f6ac151add5d1913db374ce92),elwpkh([759db348/84'/1'/0']tpubDCRMaF33e44pcJj534LXVhFbHibPbJ5vuLhSSPFAw57kYURv4tzXFL6LSnd78bkjqdmE3USedkbpXJUPA1tdzKfuYSL7PianceqAhwL2UkA/<0;1>/*))#cch6wrnp").unwrap();
    wasm_bindgen_test::console_log!("going to sync {}", descriptor);

    let network = ElementsNetwork::LiquidTestnet;
    let mut wollet = Wollet::new(network, Arc::new(NoPersist {}), descriptor).unwrap();

    let mut client = EsploraClient::new("https://blockstream.info/liquidtestnet/api");

    let update = client.full_scan(&wollet).await.unwrap().unwrap();
    wollet.apply_update(update).unwrap();
    let balance = wollet.balance().unwrap();
    wasm_bindgen_test::console_log!("balance {:?}", balance);
}

async fn fetch(url: &str) -> String {
    let client = reqwest::Client::new();
    let resp = client.get(url).send().await.unwrap();
    resp.text().await.unwrap()
}

mod tests {
    use wasm_bindgen_futures::spawn_local;
    use wasm_bindgen_test::*;

    wasm_bindgen_test_configure!(run_in_browser);

    async fn fetch(url: &str) -> String {
        let client = reqwest::Client::new();
        let resp = client.get(url).send().await.unwrap();
        resp.text().await.unwrap()
    }

    #[wasm_bindgen_test]
    fn fetch_test() {
        spawn_local(async move {
            let result = fetch("https://ifconfig.me/ip").await;
            wasm_bindgen_test::console_log!("Your IP: {}", result);
        });
    }

    #[wasm_bindgen_test]
    fn greet_test() {
        spawn_local(crate::greet());
    }
}
