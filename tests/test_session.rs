extern crate bewallet;

use bitcoin::{self, Amount, BlockHash};
use bitcoincore_rpc::{Auth, Client, RpcApi};
use chrono::Utc;
use electrum_client::raw_client::{ElectrumPlaintextStream, RawClient};
use electrum_client::ElectrumApi;
use elements;

use bewallet::error::Error;
use bewallet::model::*;
use bewallet::transaction::DUST_VALUE;
use bewallet::Config;
use bewallet::ElectrumWallet;
use bewallet::ElementsNetwork;

use log::LevelFilter;
use log::{info, warn, Metadata, Record};
use serde_json::Value;
use std::net::TcpStream;
use std::process::Child;
use std::process::Command;
use std::str::FromStr;
use std::sync::Once;
use std::thread;
use std::time::Duration;
use tempdir::TempDir;

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

fn node_sendtoaddress(
    client: &Client,
    address: &str,
    satoshi: u64,
    asset: Option<String>,
) -> String {
    let amount = Amount::from_sat(satoshi);
    let btc = amount.to_string_in(bitcoin::util::amount::Denomination::Bitcoin);
    info!("node_sendtoaddress {} {}", address, btc);
    let r = match asset {
        Some(asset) => client
            .call::<Value>(
                "sendtoaddress",
                &[
                    address.into(),
                    btc.into(),
                    "".into(),
                    "".into(),
                    false.into(),
                    false.into(),
                    1.into(),
                    "UNSET".into(),
                    asset.into(),
                ],
            )
            .unwrap(),
        None => client
            .call::<Value>("sendtoaddress", &[address.into(), btc.into()])
            .unwrap(),
    };
    info!("node_sendtoaddress result {:?}", r);
    r.as_str().unwrap().to_string()
}

fn node_getnewaddress(client: &Client, kind: Option<&str>) -> String {
    let kind = kind.unwrap_or("p2sh-segwit");
    let addr: Value = client
        .call("getnewaddress", &["label".into(), kind.into()])
        .unwrap();
    addr.as_str().unwrap().to_string()
}

fn node_generate(client: &Client, block_num: u32) {
    let address = node_getnewaddress(client, None);
    let r = client
        .call::<Value>("generatetoaddress", &[block_num.into(), address.into()])
        .unwrap();
    info!("generate result {:?}", r);
}

fn node_issueasset(client: &Client, satoshi: u64) -> String {
    let amount = Amount::from_sat(satoshi);
    let btc = amount.to_string_in(bitcoin::util::amount::Denomination::Bitcoin);
    let r = client
        .call::<Value>("issueasset", &[btc.into(), 0.into()])
        .unwrap();
    info!("node_issueasset result {:?}", r);
    r.get("asset").unwrap().as_str().unwrap().to_string()
}

fn to_unconfidential(elements_address: String) -> String {
    let mut address_unconf = elements::Address::from_str(&elements_address).unwrap();
    address_unconf.blinding_pubkey = None;
    address_unconf.to_string()
}

#[allow(unused)]
pub struct TestElectrumWallet {
    node: Client,
    electrs: RawClient<ElectrumPlaintextStream>,
    electrs_header: RawClient<ElectrumPlaintextStream>,
    electrum_wallet: ElectrumWallet,
    tx_status: u64,
    block_status: (u32, BlockHash),
    node_process: Child,
    electrs_process: Child,
    node_work_dir: TempDir,
    electrs_work_dir: TempDir,
    db_root_dir: TempDir,
    network: ElementsNetwork,
    config: Config,
    mnemonic: String,
}

// should be TestElectrumWallet::setup
pub fn setup_wallet(is_debug: bool, electrs_exec: String, node_exec: String) -> TestElectrumWallet {
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

    let node_work_dir = TempDir::new("electrum_integration_tests").unwrap();
    let node_work_dir_str = format!("{}", &node_work_dir.path().display());
    let sum_port = 1;

    let rpc_port = 55363u16 + sum_port;
    let p2p_port = 34975u16 + sum_port;
    let socket = format!("127.0.0.1:{}", rpc_port);
    let node_url = format!("http://{}", socket);

    let test = TcpStream::connect(&socket);
    assert!(
        test.is_err(),
        "check the port is not open with a previous instance of bitcoind"
    );

    let datadir_arg = format!("-datadir={}", &node_work_dir.path().display());
    let rpcport_arg = format!("-rpcport={}", rpc_port);
    let p2pport_arg = format!("-port={}", p2p_port);
    let mut args: Vec<&str> = vec![&datadir_arg, &rpcport_arg, &p2pport_arg];
    args.push("-initialfreecoins=2100000000");
    args.push("-chain=liquidregtest");
    args.push("-validatepegin=0");
    args.push("-fallbackfee=0.00001");
    if !is_debug {
        args.push("-daemon");
    }
    args.push("-dustrelayfee=0.00000001");
    info!("LAUNCHING: {} {}", node_exec, args.join(" "));
    let node_process = Command::new(node_exec).args(args).spawn().unwrap();
    info!("node spawned");

    let par_network = "liquidregtest";
    let cookie_file = node_work_dir.path().join(par_network).join(".cookie");
    // wait bitcoind is ready, use default wallet
    let mut i = 120;
    let node: Client = loop {
        assert!(i > 0, "1 minute without updates");
        i -= 1;
        thread::sleep(Duration::from_millis(500));
        assert!(node_process.stderr.is_none());
        let client_result = Client::new(node_url.clone(), Auth::CookieFile(cookie_file.clone()));
        match client_result {
            Ok(client) => match client.call::<Value>("getblockchaininfo", &[]) {
                Ok(_) => break client,
                Err(e) => warn!("{:?}", e),
            },
            Err(e) => warn!("{:?}", e),
        }
    };
    info!("Bitcoin started");
    let cookie_value = std::fs::read_to_string(&cookie_file).unwrap();

    let electrs_port = 62431u16 + sum_port;
    let electrs_work_dir = TempDir::new("electrum_integration_tests").unwrap();
    let electrs_work_dir_str = format!("{}", &electrs_work_dir.path().display());
    let electrs_url = format!("127.0.0.1:{}", electrs_port);
    let daemon_url = format!("127.0.0.1:{}", rpc_port);
    let mut args: Vec<&str> = vec![
        "--db-dir",
        &electrs_work_dir_str,
        "--daemon-dir",
        &node_work_dir_str,
        "--electrum-rpc-addr",
        &electrs_url,
        "--daemon-rpc-addr",
        &daemon_url,
        "--network",
        par_network,
        "--cookie",
        &cookie_value,
    ];
    if is_debug {
        args.push("-v");
    }

    info!("LAUNCHING: {} {}", electrs_exec, args.join(" "));
    let electrs_process = Command::new(electrs_exec).args(args).spawn().unwrap();
    info!("Electrs spawned");

    node_generate(&node, 101);

    info!("creating electrs client");
    let mut i = 120;
    let electrs_header = loop {
        assert!(i > 0, "1 minute without updates");
        i -= 1;
        match RawClient::new(&electrs_url) {
            Ok(c) => {
                let header = c.block_headers_subscribe_raw().unwrap();
                if header.height == 101 {
                    break c;
                }
            }
            Err(e) => {
                warn!("{:?}", e);
                thread::sleep(Duration::from_millis(500));
            }
        }
    };
    let electrs = RawClient::new(&electrs_url).unwrap();
    info!("done creating electrs client");

    let mut config = Config::default();
    config.electrum_url = Some(electrs_url.to_string());
    config.development = true;
    config.spv_enabled = Some(true);
    config.liquid = true;
    config.policy_asset =
        Some("5ac9f65c0efcc4775e0baec4ec03abdde22473cd3cf33c0419ca290e0751b225".into());
    let db_root_dir = TempDir::new("electrum_integration_tests").unwrap();

    let db_root = format!("{}", db_root_dir.path().display());

    info!("starting wallet electrum");
    let mnemonic = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about".to_string();
    let electrum_wallet = ElectrumWallet::new(config.clone(), &db_root, &mnemonic).unwrap();
    electrum_wallet.update_fee_estimates();

    let tx_status = electrum_wallet.tx_status().unwrap();
    assert_eq!(tx_status, 15130871412783076140);
    let mut i = 120;
    let block_status = loop {
        assert!(i > 0, "1 minute without updates");
        i -= 1;
        let block_status = electrum_wallet.block_status().unwrap();
        if block_status.0 == 101 {
            break block_status;
        } else {
            thread::sleep(Duration::from_millis(500));
        }
    };
    assert_eq!(block_status.0, 101);

    let network = ElementsNetwork::ElementsRegtest;

    info!("returning TestElectrumWallet");
    TestElectrumWallet {
        tx_status,
        block_status,
        node,
        electrs,
        electrs_header,
        electrum_wallet,
        node_process,
        electrs_process,
        node_work_dir,
        electrs_work_dir,
        db_root_dir,
        network,
        config,
        mnemonic,
    }
}

impl TestElectrumWallet {
    /// stop the bitcoin node in the test session
    pub fn stop(&mut self) {
        self.node.stop().unwrap();
        self.node_process.wait().unwrap();
        self.electrs_process.kill().unwrap();
    }

    pub fn node_getnewaddress(&self, kind: Option<&str>) -> String {
        node_getnewaddress(&self.node, kind)
    }

    fn node_sendtoaddress(&self, address: &str, satoshi: u64, asset: Option<String>) -> String {
        node_sendtoaddress(&self.node, address, satoshi, asset)
    }
    fn node_issueasset(&self, satoshi: u64) -> String {
        node_issueasset(&self.node, satoshi)
    }
    fn node_generate(&self, block_num: u32) {
        node_generate(&self.node, block_num)
    }

    /// wait wallet tx status to change (max 1 min)
    fn wallet_wait_tx_status_change(&mut self) {
        for _ in 0..120 {
            if let Ok(new_status) = self.electrum_wallet.tx_status() {
                if self.tx_status != new_status {
                    self.tx_status = new_status;
                    break;
                }
            }
            thread::sleep(Duration::from_millis(500));
        }
    }

    /// wait wallet block status to change (max 1 min)
    fn wallet_wait_block_status_change(&mut self) {
        for _ in 0..120 {
            if let Ok(new_status) = self.electrum_wallet.block_status() {
                if self.block_status != new_status {
                    self.block_status = new_status;
                    break;
                }
            }
            thread::sleep(Duration::from_millis(500));
        }
    }

    /// asset balance in satoshi
    fn balance_asset(&self, asset: Option<String>) -> u64 {
        let balance = self.electrum_wallet.balance().unwrap();
        info!("balance: {:?}", balance);
        let asset = asset.unwrap_or(self.config.policy_asset.as_ref().unwrap().to_string());
        *balance.get(&asset).unwrap_or(&0i64) as u64
    }

    fn balance_btc(&self) -> u64 {
        self.balance_asset(None)
    }

    fn get_tx_from_list(&mut self, txid: &str) -> TransactionDetails {
        self.electrum_wallet.update_spv().unwrap();
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

    pub fn fund_btc(&mut self) {
        let init_balance = self.balance_btc();
        let satoshi: u64 = 1_000_000;
        let ap = self.electrum_wallet.address().unwrap();
        let txid = self.node_sendtoaddress(&ap.address, satoshi, None);
        self.wallet_wait_tx_status_change();
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

    pub fn fund_asset(&mut self) -> String {
        let num_utxos_before = self.electrum_wallet.utxos().unwrap().len();
        let satoshi = 10_000;
        let asset = self.node_issueasset(satoshi);
        let ap = self.electrum_wallet.address().unwrap();
        let txid = self.node_sendtoaddress(&ap.address, satoshi, Some(asset.clone()));
        self.wallet_wait_tx_status_change();

        let balance_asset = self.balance_asset(Some(asset.clone()));
        assert_eq!(balance_asset, satoshi);
        let wallet_txid = self.get_tx_from_list(&txid).txid;
        assert_eq!(txid, wallet_txid);
        let utxos = self.electrum_wallet.utxos().unwrap();
        assert_eq!(utxos.len(), num_utxos_before + 1);
        asset
    }

    /// balance in satoshi of the node
    fn node_balance(&self, asset: Option<String>) -> u64 {
        let balance: Value = self.node.call("getbalance", &[]).unwrap();
        let unconfirmed_balance: Value = self.node.call("getunconfirmedbalance", &[]).unwrap();
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

    pub fn policy_asset(&self) -> Option<String> {
        self.config.policy_asset.clone()
    }

    /// send a tx from the wallet to the specified address
    pub fn send_tx(
        &mut self,
        address: &str,
        satoshi: u64,
        asset: Option<String>,
        utxos: Option<Vec<TXO>>,
    ) -> String {
        let init_sat = self.balance_asset(asset.clone());
        let init_node_balance = self.node_balance(asset.clone());
        let mut create_opt = CreateTransactionOpt::default();
        let fee_rate = 100;
        create_opt.fee_rate = Some(fee_rate);
        create_opt.addressees.push(AddressAmount {
            address: address.to_string(),
            satoshi,
            asset_tag: asset.clone().or(self.policy_asset()),
        });
        create_opt.utxos = utxos;
        let tx_details = self.electrum_wallet.create_tx(&mut create_opt).unwrap();
        let mut tx = tx_details.transaction.clone();
        let len_before = elements::encode::serialize(&tx).len();
        self.electrum_wallet
            .sign_tx(&mut tx, &self.mnemonic)
            .unwrap();
        let len_after = elements::encode::serialize(&tx).len();
        assert!(len_before < len_after, "sign tx did not increased tx size");
        //self.check_fee_rate(fee_rate, &signed_tx, MAX_FEE_PERCENT_DIFF);
        let txid = tx.txid().to_string();
        self.electrum_wallet.broadcast_tx(&tx).unwrap();
        self.wallet_wait_tx_status_change();

        self.tx_checks(&tx);

        let fee = if asset.is_none() || asset == self.config.policy_asset {
            tx_details.fee
        } else {
            0
        };
        assert_eq!(
            self.node_balance(asset.clone()),
            init_node_balance + satoshi,
            "node balance does not match"
        );

        let expected = init_sat - satoshi - fee;
        for _ in 0..5 {
            if expected != self.balance_asset(asset.clone()) {
                // FIXME I should not wait again, but apparently after reconnect it's needed
                self.wallet_wait_tx_status_change();
            }
        }
        assert_eq!(
            self.balance_asset(asset.clone()),
            expected,
            "gdk balance does not match"
        );

        //self.list_tx_contains(&txid, &vec![address.to_string()], true);
        let wallet_txid = self.get_tx_from_list(&txid).txid;
        assert_eq!(txid, wallet_txid);

        txid
    }

    pub fn send_tx_to_unconf(&mut self) {
        let init_sat = self.balance_btc();
        let ap = self.electrum_wallet.address().unwrap();
        let unconf_address = to_unconfidential(ap.address);
        self.node_sendtoaddress(&unconf_address, 10_000, None);
        self.wallet_wait_tx_status_change();
        assert_eq!(init_sat, self.balance_btc());
    }

    pub fn is_verified(&mut self, txid: &str, verified: SPVVerifyResult) {
        let tx = self.get_tx_from_list(txid);
        assert_eq!(tx.spv_verified.to_string(), verified.to_string());
    }

    pub fn send_all(&mut self, address: &str, asset: Option<String>) {
        let mut create_opt = CreateTransactionOpt::default();
        let fee_rate = 1000;
        create_opt.fee_rate = Some(fee_rate);
        create_opt.addressees.push(AddressAmount {
            address: address.to_string(),
            satoshi: 0,
            asset_tag: asset.clone(),
        });
        create_opt.send_all = Some(true);
        let tx_details = self.electrum_wallet.create_tx(&mut create_opt).unwrap();
        let mut tx = tx_details.transaction.clone();
        self.electrum_wallet
            .sign_tx(&mut tx, &self.mnemonic)
            .unwrap();

        //self.check_fee_rate(fee_rate, &signed_tx, MAX_FEE_PERCENT_DIFF);
        self.electrum_wallet.broadcast_tx(&tx).unwrap();
        self.wallet_wait_tx_status_change();
        assert_eq!(self.balance_asset(asset), 0);
    }

    /// ask the blockcain tip to electrs
    fn electrs_tip(&mut self) -> usize {
        for _ in 0..10 {
            match self.electrs_header.block_headers_subscribe_raw() {
                Ok(header) => return header.height,
                Err(e) => {
                    warn!("electrs_tip {:?}", e); // fixme, for some reason it errors once every two try
                    thread::sleep(Duration::from_millis(500));
                }
            }
        }
        panic!("electrs_tip always return error")
    }

    /// mine a block with the node and check if wallet sees the change
    pub fn mine_block(&mut self) {
        let initial_height = self.electrs_tip();
        info!("mine_block initial_height {}", initial_height);
        self.node_generate(1);
        self.wallet_wait_block_status_change();
        let mut i = 120;
        let new_height = loop {
            assert!(i > 0, "1 minute without updates");
            i -= 1;
            // apparently even if wallet status changed (thus new height come in)
            // it could happend this is the old height (maybe due to caching) thus we loop wait
            let new_height = self.electrs_tip();
            if new_height != initial_height {
                break new_height;
            }
            info!("height still the same");
            thread::sleep(Duration::from_millis(500));
        };
        info!("mine_block new_height {}", new_height);
        assert_eq!(initial_height + 1, new_height);
    }

    /// send a tx with multiple recipients with same amount from the wallet to addresses generated
    /// by the node. If `assets` contains values, they are used as asset_tag cyclically
    pub fn send_multi(&mut self, recipients: u8, amount: u64, assets: &Vec<String>) {
        let init_sat = self.balance_btc();
        let init_balances = self.electrum_wallet.balance().unwrap();
        let mut create_opt = CreateTransactionOpt::default();
        let fee_rate = 1000;
        create_opt.fee_rate = Some(fee_rate);
        let mut addressees = vec![];
        let mut assets_cycle = assets.iter().cycle();
        let mut tags = vec![];
        for _ in 0..recipients {
            let address = self.node_getnewaddress(None);
            let asset_tag = if assets.is_empty() {
                self.policy_asset()
            } else {
                let current = assets_cycle.next().unwrap().to_string();
                tags.push(current.clone());
                Some(current)
            };

            create_opt.addressees.push(AddressAmount {
                address: address.to_string(),
                satoshi: amount,
                asset_tag,
            });
            addressees.push(address);
        }
        let tx_details = self.electrum_wallet.create_tx(&mut create_opt).unwrap();
        let mut tx = tx_details.transaction.clone();
        self.electrum_wallet
            .sign_tx(&mut tx, &self.mnemonic)
            .unwrap();
        //self.check_fee_rate(fee_rate, &signed_tx, MAX_FEE_PERCENT_DIFF);
        let _txid = tx.txid().to_string();
        self.electrum_wallet.broadcast_tx(&tx).unwrap();
        self.wallet_wait_tx_status_change();
        self.tx_checks(&tx);

        let fee = tx_details.fee;
        if assets.is_empty() {
            assert_eq!(
                init_sat - fee - recipients as u64 * amount,
                self.balance_btc()
            );
        } else {
            assert_eq!(init_sat - fee, self.balance_btc());
            for tag in assets {
                let outputs_for_this_asset = tags.iter().filter(|t| t == &tag).count() as u64;
                assert_eq!(
                    *init_balances.get(tag).unwrap() as u64 - outputs_for_this_asset * amount,
                    self.balance_asset(Some(tag.to_string()))
                );
            }
        }
        //TODO check node balance
        //self.list_tx_contains(&txid, &addressees, true);
    }

    /// check create_tx failure reasons
    pub fn create_fails(&mut self) {
        let init_sat = self.balance_btc();
        let mut create_opt = CreateTransactionOpt::default();
        let fee_rate = 1000;
        let address = self.node_getnewaddress(None);
        create_opt.fee_rate = Some(fee_rate);
        create_opt.addressees.push(AddressAmount {
            address: address.to_string(),
            satoshi: 0,
            asset_tag: self.policy_asset(),
        });
        assert!(matches!(
            self.electrum_wallet.create_tx(&mut create_opt),
            Err(Error::InvalidAmount)
        ));

        create_opt.addressees[0].satoshi = 200; // below dust limit
        assert!(matches!(
            self.electrum_wallet.create_tx(&mut create_opt),
            Err(Error::InvalidAmount)
        ));

        create_opt.addressees[0].satoshi = init_sat; // not enough to pay the fee
        assert!(matches!(
            self.electrum_wallet.create_tx(&mut create_opt),
            Err(Error::InsufficientFunds)
        ));

        create_opt.addressees[0].address = "x".to_string();
        assert!(matches!(
            self.electrum_wallet.create_tx(&mut create_opt),
            Err(Error::InvalidAddress)
        ));

        create_opt.addressees[0].address = "38CMdevthTKYAtxaSkYYtcv5QgkHXdKKk5".to_string();
        assert!(
            matches!(
                self.electrum_wallet.create_tx(&mut create_opt),
                Err(Error::InvalidAddress)
            ),
            "address with different network should fail"
        );

        create_opt.addressees[0].address =
            "VJLCbLBTCdxhWyjVLdjcSmGAksVMtabYg15maSi93zknQD2ihC38R7CUd8KbDFnV8A4hiykxnRB3Uv6d"
                .to_string();
        assert!(
            matches!(
                self.electrum_wallet.create_tx(&mut create_opt),
                Err(Error::InvalidAddress)
            ),
            "address with different network should fail"
        );

        create_opt.addressees[0].address =
            "bc1pw508d6qejxtdg4y5r3zarvary0c5xw7kw508d6qejxtdg4y5r3zarvary0c5xw7k7grplx"
                .to_string(); // from bip173 test vectors
        assert!(
            matches!(
                self.electrum_wallet.create_tx(&mut create_opt),
                Err(Error::InvalidAddress)
            ),
            "segwit v1 should fail"
        );

        let addr =
            "Azpt6vXqrbPuUtsumAioGjKnvukPApDssC1HwoFdSWZaBYJrUVSe5K8x9nk2HVYiYANy9mVQbW3iQ6xU";
        let mut addr = elements::Address::from_str(addr).unwrap();
        addr.blinding_pubkey = None;
        create_opt.addressees[0].address = addr.to_string();
        assert!(
            matches!(
                self.electrum_wallet.create_tx(&mut create_opt),
                Err(Error::InvalidAddress)
            ),
            "unblinded address should fail"
        );

        create_opt.addressees.clear();
        assert!(matches!(
            self.electrum_wallet.create_tx(&mut create_opt),
            Err(Error::EmptyAddressees)
        ));
    }

    pub fn utxos(&self) -> Vec<TXO> {
        self.electrum_wallet.utxos().unwrap()
    }

    /// performs checks on transactions, like checking for address reuse in outputs and on liquid confidential commitments inequality
    pub fn tx_checks(&self, transaction: &elements::Transaction) {
        let output_nofee: Vec<&elements::TxOut> =
            transaction.output.iter().filter(|o| !o.is_fee()).collect();
        for current in output_nofee.iter() {
            assert_eq!(
                1,
                output_nofee
                    .iter()
                    .filter(|o| o.script_pubkey == current.script_pubkey)
                    .count(),
                "address reuse"
            ); // for example using the same change address for lbtc and asset change
            assert_eq!(
                1,
                output_nofee
                    .iter()
                    .filter(|o| o.asset == current.asset)
                    .count(),
                "asset commitment equal"
            );
            assert_eq!(
                1,
                output_nofee
                    .iter()
                    .filter(|o| o.value == current.value)
                    .count(),
                "value commitment equal"
            );
            assert_eq!(
                1,
                output_nofee
                    .iter()
                    .filter(|o| o.nonce == current.nonce)
                    .count(),
                "nonce commitment equal"
            );
        }
        assert!(
            transaction.output.last().unwrap().is_fee(),
            "last output is not a fee"
        );
    }
}
