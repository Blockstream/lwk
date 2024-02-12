mod utils;

use std::{str::FromStr, sync::Arc};

use lwk_wollet::{
    BlockchainBackend, ElementsNetwork, EsploraClient, NoPersist, Wollet, WolletDescriptor,
};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
extern "C" {
    fn alert(s: &str);
}

#[wasm_bindgen]
pub fn greet() {
    let descriptor = WolletDescriptor::from_str("ct(slip77(ab5824f4477b4ebb00a132adfd8eb0b7935cf24f6ac151add5d1913db374ce92),elwpkh([759db348/84'/1'/0']tpubDCRMaF33e44pcJj534LXVhFbHibPbJ5vuLhSSPFAw57kYURv4tzXFL6LSnd78bkjqdmE3USedkbpXJUPA1tdzKfuYSL7PianceqAhwL2UkA/<0;1>/*))#cch6wrnp").unwrap();
    alert(&format!("going to sync {}", descriptor));

    let network = ElementsNetwork::LiquidTestnet;
    let mut wollet = Wollet::new(network, Arc::new(NoPersist {}), descriptor).unwrap();

    let mut client = EsploraClient::new("https://blockstream.info/liquidtestnet/api");
    let update = client.full_scan(&wollet).unwrap().unwrap(); // TODO blocks here
    wollet.apply_update(update).unwrap();
    let balance = wollet.balance().unwrap();

    alert(&format!("balance {:?}", balance));
}
