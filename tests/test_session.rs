extern crate bewallet;

use electrsd::bitcoind::bitcoincore_rpc::{Auth, Client, RpcApi};
use electrum_client::ElectrumApi;
use elements::bitcoin::util::amount::Denomination;
use elements::bitcoin::Amount;
use elements::{Address, AssetId, BlockHash};

use bewallet::*;

use chrono::Utc;
use log::LevelFilter;
use log::{info, warn, Metadata, Record};
use serde_json::Value;
use std::str::FromStr;
use std::sync::Once;
use std::thread;
use std::time::Duration;
use tempdir::TempDir;

const DUST_VALUE: u64 = 546;

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

fn node_getnewaddress(client: &Client, kind: Option<&str>) -> Address {
    let kind = kind.unwrap_or("p2sh-segwit");
    let addr: Value = client
        .call("getnewaddress", &["label".into(), kind.into()])
        .unwrap();
    Address::from_str(&addr.as_str().unwrap()).unwrap()
}

fn node_generate(client: &Client, block_num: u32) {
    let address = node_getnewaddress(client, None).to_string();
    let r = client
        .call::<Value>("generatetoaddress", &[block_num.into(), address.into()])
        .unwrap();
    info!("generate result {:?}", r);
}

pub struct TestElectrumServer {
    node: electrsd::bitcoind::BitcoinD,
    pub electrs: electrsd::ElectrsD,
}

impl TestElectrumServer {
    pub fn new(is_debug: bool, electrs_exec: String, node_exec: String) -> Self {
        START.call_once(|| {
            let filter = if is_debug {
                LevelFilter::Info
            } else {
                LevelFilter::Off
            };
            log::set_logger(&LOGGER)
                .map(|()| log::set_max_level(filter))
                .expect("cannot initialize logging");
        });

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
        conf.view_stdout = is_debug;
        conf.p2p = electrsd::bitcoind::P2P::Yes;
        conf.network = network;

        let node = electrsd::bitcoind::BitcoinD::with_conf(&node_exec, &conf).unwrap();
        info!("node spawned");

        node_generate(&node.client, 1);
        // send initialfreecoins from wallet "" to the wallet created by BitcoinD::new
        let node_url = format!("http://127.0.0.1:{}/wallet/", node.params.rpc_socket.port());
        let client =
            Client::new(&node_url, Auth::CookieFile(node.params.cookie_file.clone())).unwrap();
        let address = node_getnewaddress(&node.client, None);
        client
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

        let args = if is_debug { vec!["-v"] } else { vec![] };
        let mut conf = electrsd::Conf::default();
        conf.args = args;
        conf.view_stderr = is_debug;
        conf.http_enabled = false;
        conf.network = network;
        let electrs = electrsd::ElectrsD::with_conf(&electrs_exec, &node, &conf).unwrap();
        info!("Electrs spawned");

        node_generate(&node.client, 100);
        electrs.trigger().unwrap();

        let mut i = 120;
        loop {
            assert!(i > 0, "1 minute without updates");
            i -= 1;
            let height = electrs.client.block_headers_subscribe_raw().unwrap().height;
            if height == 101 {
                break;
            } else {
                warn!("height: {}", height);
            }
            thread::sleep(Duration::from_millis(500));
        }
        info!("Electrs synced with node");

        Self { node, electrs }
    }

    /// stop the bitcoin node in the test session
    pub fn stop(&mut self) {
        self.node.client.stop().unwrap();
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
        let txid = self.node_sendtoaddress(address, satoshi, None);
        txid
    }

    pub fn fund_asset(&mut self, address: &Address, satoshi: u64) -> (String, AssetId) {
        let asset = self.node_issueasset(satoshi);
        let txid = self.node_sendtoaddress(address, satoshi, Some(asset.clone()));
        (txid, asset)
    }

    /// balance in satoshi of the node
    fn _node_balance(&self, asset: Option<String>) -> u64 {
        let balance: Value = self.node.client.call("getbalance", &[]).unwrap();
        let unconfirmed_balance: Value =
            self.node.client.call("getunconfirmedbalance", &[]).unwrap();
        let asset_or_policy = asset.or(Some("bitcoin".to_string())).unwrap();
        let balance = match balance.get(&asset_or_policy) {
            Some(Value::Number(s)) => s.as_f64().unwrap(),
            _ => 0.0,
        };
        let unconfirmed_balance = match unconfirmed_balance.get(&asset_or_policy) {
            Some(Value::Number(s)) => s.as_f64().unwrap(),
            _ => 0.0,
        };
        ((balance + unconfirmed_balance) * 100_000_000.0) as u64
    }
}

pub struct TestElectrumWallet {
    _mnemonic: String,
    electrum_wallet: ElectrumWallet,
    _tx_status: u64,
    _block_status: (u32, BlockHash),
    _db_root_dir: TempDir,
}

impl TestElectrumWallet {
    pub fn new(electrs_url: &str, _mnemonic: String) -> Self {
        let tls = false;
        let validate_domain = false;
        let policy_asset_hex = &"5ac9f65c0efcc4775e0baec4ec03abdde22473cd3cf33c0419ca290e0751b225";
        let _db_root_dir = TempDir::new("electrum_integration_tests").unwrap();

        let db_root = format!("{}", _db_root_dir.path().display());

        let electrum_wallet = ElectrumWallet::new_regtest(
            policy_asset_hex,
            electrs_url,
            tls,
            validate_domain,
            &db_root,
            &_mnemonic,
        )
        .unwrap();

        let _tx_status = electrum_wallet.tx_status().unwrap();
        assert_eq!(_tx_status, 15130871412783076140);
        let mut i = 120;
        let _block_status = loop {
            assert!(i > 0, "1 minute without updates");
            i -= 1;
            let block_status = electrum_wallet.block_status().unwrap();
            if block_status.0 == 101 {
                break block_status;
            } else {
                thread::sleep(Duration::from_millis(500));
            }
        };
        assert_eq!(_block_status.0, 101);

        Self {
            _mnemonic,
            electrum_wallet,
            _tx_status,
            _block_status,
            _db_root_dir,
        }
    }

    /// Wait until tx appears in tx list (max 1 min)
    fn wait_for_tx(&mut self, txid: &str) {
        let mut opt = GetTransactionsOpt::default();
        opt.count = 100;
        for _ in 0..120 {
            let list = self.electrum_wallet.transactions(&opt).unwrap();
            if list.iter().any(|e| e.txid == txid) {
                return;
            }
            thread::sleep(Duration::from_millis(500));
        }
        panic!("Wallet does not have {} in its list", txid);
    }

    /// asset balance in satoshi
    fn balance(&self, asset: &AssetId) -> u64 {
        let balance = self.electrum_wallet.balance().unwrap();
        info!("balance: {:?}", balance);
        *balance.get(asset).unwrap_or(&0u64)
    }

    fn balance_btc(&self) -> u64 {
        self.balance(&self.electrum_wallet.policy_asset())
    }

    fn get_tx_from_list(&mut self, txid: &str) -> TransactionDetails {
        let mut opt = GetTransactionsOpt::default();
        opt.count = 100;
        let list = self.electrum_wallet.transactions(&opt).unwrap();
        let filtered_list: Vec<TransactionDetails> =
            list.iter().filter(|e| e.txid == txid).cloned().collect();
        assert!(
            !filtered_list.is_empty(),
            "just made tx {} is not in tx list",
            txid
        );
        filtered_list.first().unwrap().clone()
    }

    pub fn fund_btc(&mut self, server: &mut TestElectrumServer) {
        let init_balance = self.balance_btc();
        let satoshi: u64 = 1_000_000;
        let address = self.electrum_wallet.address().unwrap();
        let txid = server.fund_btc(&address, satoshi);
        self.wait_for_tx(&txid);
        let balance = init_balance + self.balance_btc();
        // node is allowed to make tx below dust with dustrelayfee, but wallet should not see
        // this as spendable, thus the balance should not change
        let satoshi = if satoshi < DUST_VALUE {
            init_balance
        } else {
            init_balance + satoshi
        };
        assert_eq!(balance, satoshi);
        let wallet_txid = self.get_tx_from_list(&txid).txid;
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
        let wallet_txid = self.get_tx_from_list(&txid).txid;
        assert_eq!(txid, wallet_txid);
        let utxos = self.electrum_wallet.utxos().unwrap();
        assert_eq!(utxos.len(), num_utxos_before + 1);
        asset
    }
}
