extern crate wollet;

use chrono::Utc;
use electrsd::bitcoind::bitcoincore_rpc::{Client, RpcApi};
use electrum_client::ElectrumApi;
use elements::bitcoin::amount::Denomination;
use elements::bitcoin::Amount;
use elements::pset::PartiallySignedTransaction;
use elements::{Address, AssetId, Transaction};
use elements_miniscript::descriptor::checksum::desc_checksum;
use log::{LevelFilter, Metadata, Record};
use serde_json::Value;
use std::env;
use std::str::FromStr;
use std::sync::Once;
use std::thread;
use std::time::Duration;
use tempdir::TempDir;
use wollet::*;

static LOGGER: SimpleLogger = SimpleLogger;

//TODO duplicated why I cannot import?
pub struct SimpleLogger;

impl log::Log for SimpleLogger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level() <= log::max_level()
    }

    fn log(&self, record: &Record) {
        if self.enabled(record.metadata()) {
            println!(
                "{} {} - {}",
                Utc::now().format("%S%.3f"),
                record.level(),
                record.args()
            );
        }
    }

    fn flush(&self) {}
}

static START: Once = Once::new();

fn add_checksum(desc: &str) -> String {
    if desc.find('#').is_some() {
        desc.into()
    } else {
        format!("{}#{}", desc, desc_checksum(desc).unwrap())
    }
}

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
            .unwrap_or(LevelFilter::Off);
        START.call_once(|| {
            log::set_logger(&LOGGER)
                .map(|()| log::set_max_level(filter))
                .expect("cannot initialize logging");
        });
        let view_stdout = filter != LevelFilter::Off;

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

    fn node_sendtoaddress(
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

    fn node_issueasset(&self, satoshi: u64) -> AssetId {
        let amount = Amount::from_sat(satoshi);
        let btc = amount.to_string_in(Denomination::Bitcoin);
        let r = self
            .node
            .client
            .call::<Value>("issueasset", &[btc.into(), 0.into()])
            .unwrap();
        let asset = r.get("asset").unwrap().as_str().unwrap().to_string();
        AssetId::from_str(&asset).unwrap()
    }

    pub fn fund_btc(&mut self, address: &Address, satoshi: u64) -> String {
        self.node_sendtoaddress(address, satoshi, None)
    }

    pub fn fund_asset(&mut self, address: &Address, satoshi: u64) -> (String, AssetId) {
        let asset = self.node_issueasset(satoshi);
        let txid = self.node_sendtoaddress(address, satoshi, Some(asset));
        (txid, asset)
    }
}

pub struct TestElectrumWallet {
    electrum_wallet: ElectrumWallet,
    _db_root_dir: TempDir,
}

impl TestElectrumWallet {
    pub fn new(electrs_url: &str, desc: &str) -> Self {
        let tls = false;
        let validate_domain = false;
        let policy_asset =
            AssetId::from_str("5ac9f65c0efcc4775e0baec4ec03abdde22473cd3cf33c0419ca290e0751b225")
                .unwrap();
        let _db_root_dir = TempDir::new("electrum_integration_tests").unwrap();

        let db_root = format!("{}", _db_root_dir.path().display());

        let electrum_wallet = ElectrumWallet::new(
            ElementsNetwork::ElementsRegtest { policy_asset },
            electrs_url,
            tls,
            validate_domain,
            &db_root,
            &add_checksum(desc),
        )
        .unwrap();

        electrum_wallet.sync_txs().unwrap();
        let list = electrum_wallet.transactions().unwrap();
        assert_eq!(list.len(), 0);
        let mut i = 120;
        let tip = loop {
            assert!(i > 0, "1 minute without updates");
            i -= 1;
            electrum_wallet.sync_tip().unwrap();
            let tip = electrum_wallet.tip().unwrap();
            if tip.0 == 101 {
                break tip.0;
            } else {
                thread::sleep(Duration::from_millis(500));
            }
        };
        assert_eq!(tip, 101);

        Self {
            electrum_wallet,
            _db_root_dir,
        }
    }

    /// Wait until tx appears in tx list (max 1 min)
    fn wait_for_tx(&mut self, txid: &str) {
        for _ in 0..120 {
            self.electrum_wallet.sync_txs().unwrap();
            let list = self.electrum_wallet.transactions().unwrap();
            if list.iter().any(|e| e.0.txid().to_string() == txid) {
                return;
            }
            thread::sleep(Duration::from_millis(500));
        }
        panic!("Wallet does not have {} in its list", txid);
    }

    /// asset balance in satoshi
    fn balance(&self, asset: &AssetId) -> u64 {
        self.electrum_wallet.sync_txs().unwrap();
        let balance = self.electrum_wallet.balance().unwrap();
        *balance.get(asset).unwrap_or(&0u64)
    }

    fn balance_btc(&self) -> u64 {
        self.balance(&self.electrum_wallet.policy_asset())
    }

    fn get_tx_from_list(&mut self, txid: &str) -> Transaction {
        self.electrum_wallet.sync_txs().unwrap();
        let list = self.electrum_wallet.transactions().unwrap();
        let filtered_list: Vec<_> = list
            .iter()
            .filter(|e| e.0.txid().to_string() == txid)
            .cloned()
            .collect();
        assert!(
            !filtered_list.is_empty(),
            "just made tx {} is not in tx list",
            txid
        );
        filtered_list.first().unwrap().clone().0
    }

    pub fn fund_btc(&mut self, server: &mut TestElectrumServer) {
        let init_balance = self.balance_btc();
        let satoshi: u64 = 1_000_000;
        let address = self.electrum_wallet.address().unwrap();
        let txid = server.fund_btc(&address, satoshi);
        self.wait_for_tx(&txid);
        let balance = init_balance + self.balance_btc();
        let satoshi = init_balance + satoshi;
        assert_eq!(balance, satoshi);
        let wallet_txid = self.get_tx_from_list(&txid).txid().to_string();
        assert_eq!(txid, wallet_txid);
        let utxos = self.electrum_wallet.utxos().unwrap();
        assert_eq!(utxos.len(), 1);
    }

    pub fn fund_asset(&mut self, server: &mut TestElectrumServer) -> AssetId {
        let num_utxos_before = self.electrum_wallet.utxos().unwrap().len();
        let satoshi = 10_000;
        let address = self.electrum_wallet.address().unwrap();
        let (txid, asset) = server.fund_asset(&address, satoshi);
        self.wait_for_tx(&txid);

        let balance_asset = self.balance(&asset);
        assert_eq!(balance_asset, satoshi);
        let wallet_txid = self.get_tx_from_list(&txid).txid().to_string();
        assert_eq!(txid, wallet_txid);
        let utxos = self.electrum_wallet.utxos().unwrap();
        assert_eq!(utxos.len(), num_utxos_before + 1);
        asset
    }

    pub fn send_btc(&mut self) -> PartiallySignedTransaction {
        let satoshi: u64 = 10_000;
        let address = self.electrum_wallet.address().unwrap();
        self.electrum_wallet
            .sendlbtc(satoshi, &address.to_string())
            .unwrap()
    }
}

pub fn setup() -> TestElectrumServer {
    let electrs_exec = env::var("ELECTRS_LIQUID_EXEC").expect("set ELECTRS_LIQUID_EXEC");
    let node_exec = env::var("ELEMENTSD_EXEC").expect("set ELEMENTSD_EXEC");
    TestElectrumServer::new(electrs_exec, node_exec)
}
