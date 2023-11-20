use electrsd::bitcoind::bitcoincore_rpc::{Client, RpcApi};
use electrum_client::ElectrumApi;
use elements::bitcoin::amount::Denomination;
use elements::bitcoin::Amount;
use elements::pset::PartiallySignedTransaction;
use elements::{Address, AssetId, TxOutWitness};
use serde_json::Value;
use std::env;
use std::str::FromStr;
use std::sync::Once;
use std::thread;
use std::time::Duration;
use tracing::metadata::LevelFilter;

static TRACING_INIT: Once = Once::new();

fn node_getnewaddress(client: &Client, kind: Option<&str>) -> Address {
    let kind = kind.unwrap_or("p2sh-segwit");
    let addr: Value = client
        .call("getnewaddress", &["label".into(), kind.into()])
        .unwrap();
    Address::from_str(addr.as_str().unwrap()).unwrap()
}

fn node_generate(client: &Client, block_num: u32) {
    let address = node_getnewaddress(client, None).to_string();
    client
        .call::<Value>("generatetoaddress", &[block_num.into(), address.into()])
        .unwrap();
}

pub struct TestElectrumServer {
    node: electrsd::bitcoind::BitcoinD,
    pub electrs: electrsd::ElectrsD,
}

impl TestElectrumServer {
    pub fn new(electrs_exec: String, node_exec: String) -> Self {
        let filter = LevelFilter::from_str(&std::env::var("RUST_LOG").unwrap_or("off".to_string()))
            .unwrap_or(LevelFilter::OFF);

        init_logging();

        let view_stdout = filter == LevelFilter::TRACE;

        let args = vec![
            "-fallbackfee=0.0001",
            "-dustrelayfee=0.00000001",
            "-chain=liquidregtest",
            "-initialfreecoins=2100000000",
            "-validatepegin=0",
        ];
        let network = "liquidregtest";

        let mut conf = electrsd::bitcoind::Conf::default();
        conf.args = args;
        conf.view_stdout = view_stdout;
        conf.p2p = electrsd::bitcoind::P2P::Yes;
        conf.network = network;

        let node = electrsd::bitcoind::BitcoinD::with_conf(&node_exec, &conf).unwrap();

        node_generate(&node.client, 1);
        node.client.call::<Value>("rescanblockchain", &[]).unwrap();
        // send initialfreecoins to the node wallet
        let address = node_getnewaddress(&node.client, None);
        node.client
            .call::<Value>(
                "sendtoaddress",
                &[
                    address.to_string().into(),
                    "21".into(),
                    "".into(),
                    "".into(),
                    true.into(),
                ],
            )
            .unwrap();

        let args = if view_stdout { vec!["-v"] } else { vec![] };
        let mut conf = electrsd::Conf::default();
        conf.args = args;
        conf.view_stderr = view_stdout;
        conf.http_enabled = false;
        conf.network = network;
        let electrs = electrsd::ElectrsD::with_conf(&electrs_exec, &node, &conf).unwrap();

        node_generate(&node.client, 100);
        electrs.trigger().unwrap();

        let mut i = 120;
        loop {
            assert!(i > 0, "1 minute without updates");
            i -= 1;
            let height = electrs.client.block_headers_subscribe_raw().unwrap().height;
            if height == 101 {
                break;
            }
            thread::sleep(Duration::from_millis(500));
        }

        Self { node, electrs }
    }

    pub fn generate(&self, blocks: u32) {
        node_generate(&self.node.client, blocks);
    }

    pub fn node_sendtoaddress(
        &self,
        address: &Address,
        satoshi: u64,
        asset: Option<AssetId>,
    ) -> String {
        let amount = Amount::from_sat(satoshi);
        let btc = amount.to_string_in(Denomination::Bitcoin);
        let r = match asset {
            Some(asset) => self
                .node
                .client
                .call::<Value>(
                    "sendtoaddress",
                    &[
                        address.to_string().into(),
                        btc.into(),
                        "".into(),
                        "".into(),
                        false.into(),
                        false.into(),
                        1.into(),
                        "UNSET".into(),
                        false.into(),
                        asset.to_string().into(),
                    ],
                )
                .unwrap(),
            None => self
                .node
                .client
                .call::<Value>("sendtoaddress", &[address.to_string().into(), btc.into()])
                .unwrap(),
        };
        r.as_str().unwrap().to_string()
    }

    pub fn node_getnewaddress(&self) -> Address {
        node_getnewaddress(&self.node.client, None)
    }
}

pub fn setup() -> TestElectrumServer {
    let electrs_exec = env::var("ELECTRS_LIQUID_EXEC").expect("set ELECTRS_LIQUID_EXEC");
    let node_exec = env::var("ELEMENTSD_EXEC").expect("set ELEMENTSD_EXEC");
    TestElectrumServer::new(electrs_exec, node_exec)
}

fn init_logging() {
    use tracing_subscriber::prelude::*;

    TRACING_INIT.call_once(|| {
        tracing_subscriber::registry()
            .with(tracing_subscriber::fmt::layer())
            .with(tracing_subscriber::EnvFilter::from_default_env())
            .init();

        tracing::info!("logging initialized");
    });
}

#[allow(dead_code)]
pub fn prune_proofs(pset: &PartiallySignedTransaction) -> PartiallySignedTransaction {
    let mut pset = pset.clone();
    for i in pset.inputs_mut() {
        if let Some(utxo) = &mut i.witness_utxo {
            utxo.witness = TxOutWitness::default();
        }
        if let Some(tx) = &mut i.non_witness_utxo {
            tx.output
                .iter_mut()
                .for_each(|o| o.witness = Default::default());
        }
    }
    for o in pset.outputs_mut() {
        o.value_rangeproof = None;
        o.asset_surjection_proof = None;
        o.blind_value_proof = None;
        o.blind_asset_proof = None;
    }
    pset
}
