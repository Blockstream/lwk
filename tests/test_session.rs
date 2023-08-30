extern crate bewallet;

use chrono::Utc;
use electrsd::bitcoind::bitcoincore_rpc::{Auth, Client, RpcApi};
use electrum_client::ElectrumApi;
use elements;
use elements::bitcoin::hashes::hex::{FromHex, ToHex};
use elements::bitcoin::Amount;
use elements::BlockHash;

use bewallet::*;

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

fn node_sendtoaddress(
    client: &Client,
    address: &elements::Address,
    satoshi: u64,
    asset: Option<elements::issuance::AssetId>,
) -> String {
    let amount = Amount::from_sat(satoshi);
    let btc = amount.to_string_in(elements::bitcoin::util::amount::Denomination::Bitcoin);
    info!("node_sendtoaddress {} {}", address, btc);
    let r = match asset {
        Some(asset) => client
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
        None => client
            .call::<Value>("sendtoaddress", &[address.to_string().into(), btc.into()])
            .unwrap(),
    };
    info!("node_sendtoaddress result {:?}", r);
    r.as_str().unwrap().to_string()
}

fn node_getnewaddress(client: &Client, kind: Option<&str>) -> elements::Address {
    let kind = kind.unwrap_or("p2sh-segwit");
    let addr: Value = client
        .call("getnewaddress", &["label".into(), kind.into()])
        .unwrap();
    elements::Address::from_str(&addr.as_str().unwrap()).unwrap()
}

fn node_generate(client: &Client, block_num: u32) {
    let address = node_getnewaddress(client, None).to_string();
    let r = client
        .call::<Value>("generatetoaddress", &[block_num.into(), address.into()])
        .unwrap();
    info!("generate result {:?}", r);
}

fn node_issueasset(client: &Client, satoshi: u64) -> String {
    let amount = Amount::from_sat(satoshi);
    let btc = amount.to_string_in(elements::bitcoin::util::amount::Denomination::Bitcoin);
    let r = client
        .call::<Value>("issueasset", &[btc.into(), 0.into()])
        .unwrap();
    info!("node_issueasset result {:?}", r);
    r.get("asset").unwrap().as_str().unwrap().to_string()
}

fn to_unconfidential(address: &elements::Address) -> elements::Address {
    let mut address_unconf = address.clone();
    address_unconf.blinding_pubkey = None;
    address_unconf
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

    pub fn node_getnewaddress(&self, kind: Option<&str>) -> elements::Address {
        node_getnewaddress(&self.node.client, kind)
    }

    fn node_sendtoaddress(
        &self,
        address: &elements::Address,
        satoshi: u64,
        asset: Option<elements::issuance::AssetId>,
    ) -> String {
        node_sendtoaddress(&self.node.client, address, satoshi, asset)
    }
    fn node_issueasset(&self, satoshi: u64) -> elements::issuance::AssetId {
        let asset = node_issueasset(&self.node.client, satoshi);
        elements::issuance::AssetId::from_hex(&asset).unwrap()
    }
    fn node_generate(&self, block_num: u32) {
        node_generate(&self.node.client, block_num);
        self.electrs.trigger().unwrap();
    }

    pub fn fund_btc(&mut self, address: &elements::Address, satoshi: u64) -> String {
        let txid = self.node_sendtoaddress(address, satoshi, None);
        txid
    }

    pub fn fund_asset(
        &mut self,
        address: &elements::Address,
        satoshi: u64,
    ) -> (String, elements::issuance::AssetId) {
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

    pub fn send_tx_to_unconf(&mut self, address: &elements::Address) -> String {
        let unconf_address = to_unconfidential(address);
        let txid = self.node_sendtoaddress(&unconf_address, 10_000, None);
        txid
    }

    /// ask the blockcain tip to electrs
    fn electrs_tip(&mut self) -> usize {
        for _ in 0..10 {
            match self.electrs.client.block_headers_subscribe_raw() {
                Ok(header) => return header.height,
                Err(e) => {
                    warn!("electrs_tip {:?}", e); // fixme, for some reason it errors once every two try
                    thread::sleep(Duration::from_millis(500));
                }
            }
        }
        panic!("electrs_tip always return error")
    }

    /// mine a block with the node
    pub fn mine_block(&mut self) -> u32 {
        let initial_height = self.electrs_tip();
        info!("mine_block initial_height {}", initial_height);
        self.node_generate(1);
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
        new_height as u32
    }
}

pub struct TestElectrumWallet {
    mnemonic: String,
    electrum_wallet: ElectrumWallet,
    tx_status: u64,
    _block_status: (u32, BlockHash),
    _db_root_dir: TempDir,
}

impl TestElectrumWallet {
    pub fn new(electrs_url: &str, mnemonic: String) -> Self {
        let tls = false;
        let validate_domain = false;
        let spv_enabled = true;
        let policy_asset_hex = &"5ac9f65c0efcc4775e0baec4ec03abdde22473cd3cf33c0419ca290e0751b225";
        let _db_root_dir = TempDir::new("electrum_integration_tests").unwrap();

        let db_root = format!("{}", _db_root_dir.path().display());

        let electrum_wallet = ElectrumWallet::new_regtest(
            policy_asset_hex,
            electrs_url,
            tls,
            validate_domain,
            spv_enabled,
            &db_root,
            &mnemonic,
        )
        .unwrap();
        electrum_wallet.update_fee_estimates();

        let tx_status = electrum_wallet.tx_status().unwrap();
        assert_eq!(tx_status, 15130871412783076140);
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
            mnemonic,
            electrum_wallet,
            tx_status,
            _block_status,
            _db_root_dir,
        }
    }

    pub fn policy_asset(&self) -> elements::issuance::AssetId {
        self.electrum_wallet.policy_asset()
    }

    /// Wait until tx appears in tx list (max 1 min)
    pub fn wait_for_tx(&mut self, txid: &str) {
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
    fn _wallet_wait_block_status_change(&mut self) {
        for _ in 0..120 {
            if let Ok(new_status) = self.electrum_wallet.block_status() {
                if self._block_status != new_status {
                    self._block_status = new_status;
                    break;
                }
            }
            thread::sleep(Duration::from_millis(500));
        }
    }

    /// wait until wallet has a certain blockheight (max 1 min)
    pub fn wait_for_block(&mut self, new_height: u32) {
        for _ in 0..120 {
            if let Ok((height, _)) = self.electrum_wallet.block_status() {
                if height == new_height {
                    break;
                }
            }
            thread::sleep(Duration::from_millis(500));
        }
    }

    /// asset balance in satoshi
    pub fn balance(&self, asset: &elements::issuance::AssetId) -> u64 {
        let balance = self.electrum_wallet.balance().unwrap();
        info!("balance: {:?}", balance);
        *balance.get(asset).unwrap_or(&0u64)
    }

    fn balance_btc(&self) -> u64 {
        self.balance(&self.policy_asset())
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

    pub fn fund_btc(&mut self, server: &mut TestElectrumServer) {
        let init_balance = self.balance_btc();
        let satoshi: u64 = 1_000_000;
        let address = self.electrum_wallet.address().unwrap();
        let txid = server.fund_btc(&address, satoshi);
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

    pub fn fund_asset(&mut self, server: &mut TestElectrumServer) -> elements::issuance::AssetId {
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

    /// send a tx from the wallet to the specified address
    pub fn send_tx(
        &mut self,
        address: &elements::Address,
        satoshi: u64,
        asset: Option<elements::issuance::AssetId>,
        utxos: Option<Vec<UnblindedTXO>>,
    ) -> String {
        let asset = asset.unwrap_or(self.policy_asset());
        let init_sat = self.balance(&asset);
        //let init_node_balance = self.node_balance(asset.clone());
        let mut create_opt = CreateTransactionOpt::default();
        let fee_rate = 100;
        create_opt.fee_rate = Some(fee_rate);
        let net = self.electrum_wallet.network();
        create_opt.addressees.push(
            Destination::new(&address.to_string(), satoshi, &asset.to_string(), net).unwrap(),
        );
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

        let fee = if asset == self.policy_asset() {
            tx_details.fee
        } else {
            0
        };
        //assert_eq!(
        //    self.node_balance(asset.clone()),
        //    init_node_balance + satoshi,
        //    "node balance does not match"
        //);

        let expected = init_sat - satoshi - fee;
        for _ in 0..5 {
            if expected != self.balance(&asset) {
                // FIXME I should not wait again, but apparently after reconnect it's needed
                self.wallet_wait_tx_status_change();
            }
        }
        assert_eq!(self.balance(&asset), expected, "gdk balance does not match");

        //self.list_tx_contains(&txid, &vec![address.to_string()], true);
        let wallet_txid = self.get_tx_from_list(&txid).txid;
        assert_eq!(txid, wallet_txid);

        txid
    }

    pub fn send_tx_to_unconf(&mut self, server: &mut TestElectrumServer) {
        let init_sat = self.balance_btc();
        let address = self.electrum_wallet.address().unwrap();
        server.send_tx_to_unconf(&address);
        self.wallet_wait_tx_status_change();
        assert_eq!(init_sat, self.balance_btc());
    }

    pub fn is_verified(&mut self, txid: &str, verified: SPVVerifyResult) {
        let tx = self.get_tx_from_list(txid);
        assert_eq!(tx.spv_verified.to_string(), verified.to_string());
    }

    /// send a tx with multiple recipients with same amount from the wallet to addresses generated
    /// by the node. If `assets` contains values, they are used as asset cyclically
    pub fn send_multi(
        &mut self,
        recipients: u8,
        amount: u64,
        assets: &Vec<elements::issuance::AssetId>,
        server: &mut TestElectrumServer,
    ) {
        let init_sat = self.balance_btc();
        let init_balances = self.electrum_wallet.balance().unwrap();
        let mut create_opt = CreateTransactionOpt::default();
        let fee_rate = 1000;
        create_opt.fee_rate = Some(fee_rate);
        let net = self.electrum_wallet.network();
        let mut addressees = vec![];
        let mut assets_cycle = assets.iter().cycle();
        let mut tags = vec![];
        for _ in 0..recipients {
            let address = server.node_getnewaddress(None);
            let asset = if assets.is_empty() {
                self.policy_asset()
            } else {
                let current = elements::issuance::AssetId::from_hex(
                    &assets_cycle.next().unwrap().to_string(),
                )
                .unwrap();
                tags.push(current);
                current
            };
            create_opt.addressees.push(
                Destination::new(&address.to_string(), amount, &asset.to_hex(), net).unwrap(),
            );
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
            for asset in assets {
                let outputs_for_this_asset = tags.iter().filter(|t| t == &asset).count() as u64;
                assert_eq!(
                    *init_balances.get(&asset).unwrap() as u64 - outputs_for_this_asset * amount,
                    self.balance(asset)
                );
            }
        }
        //TODO check node balance
        //self.list_tx_contains(&txid, &addressees, true);
    }

    /// check create_tx failure reasons
    pub fn create_fails(&mut self, server: &mut TestElectrumServer) {
        let policy_asset = self.policy_asset();
        let init_sat = self.balance_btc();
        let mut create_opt = CreateTransactionOpt::default();
        let fee_rate = 1000;
        let address = server.node_getnewaddress(None).to_string();
        create_opt.fee_rate = Some(fee_rate);
        let net = self.electrum_wallet.network();
        create_opt.addressees =
            vec![Destination::new(&address, 0, &policy_asset.to_hex(), net).unwrap()];
        assert!(matches!(
            self.electrum_wallet.create_tx(&mut create_opt),
            Err(Error::InvalidAmount)
        ));

        create_opt.addressees =
            vec![Destination::new(&address, 200, &policy_asset.to_hex(), net).unwrap()];
        assert!(matches!(
            self.electrum_wallet.create_tx(&mut create_opt),
            Err(Error::InvalidAmount)
        ));

        create_opt.addressees = vec![Destination::new(
            &address,
            init_sat, // not enough to pay fee
            &policy_asset.to_hex(),
            net,
        )
        .unwrap()];
        assert!(matches!(
            self.electrum_wallet.create_tx(&mut create_opt),
            Err(Error::InsufficientFunds)
        ));

        assert!(matches!(
            Destination::new("x", 200, &policy_asset.to_hex(), net),
            Err(Error::InvalidAddress)
        ));

        assert!(
            matches!(
                Destination::new(
                    "38CMdevthTKYAtxaSkYYtcv5QgkHXdKKk5",
                    200,
                    &policy_asset.to_hex(),
                    net,
                ),
                Err(Error::InvalidAddress)
            ),
            "address with different network should fail"
        );

        assert!(
            matches!(
                Destination::new("VJLCbLBTCdxhWyjVLdjcSmGAksVMtabYg15maSi93zknQD2ihC38R7CUd8KbDFnV8A4hiykxnRB3Uv6d", 200, &policy_asset.to_hex(), net),
                Err(Error::InvalidAddress)
            ),
            "address with different network should fail"
        );

        // from bip173 test vectors
        assert!(
            matches!(
                Destination::new(
                    "bc1pw508d6qejxtdg4y5r3zarvary0c5xw7kw508d6qejxtdg4y5r3zarvary0c5xw7k7grplx",
                    200,
                    &policy_asset.to_hex(),
                    net,
                ),
                Err(Error::InvalidAddress)
            ),
            "segwit v1 should fail"
        );

        let mut addr = elements::Address::from_str(
            "Azpt6vXqrbPuUtsumAioGjKnvukPApDssC1HwoFdSWZaBYJrUVSe5K8x9nk2HVYiYANy9mVQbW3iQ6xU",
        )
        .unwrap();
        addr.blinding_pubkey = None;
        create_opt.addressees =
            vec![Destination::new(&addr.to_string(), 1000, &policy_asset.to_hex(), net).unwrap()];
        assert!(
            matches!(
                self.electrum_wallet.create_tx(&mut create_opt),
                Err(Error::InvalidAddress)
            ),
            "unblinded address should fail"
        );

        create_opt.addressees = vec![];
        assert!(matches!(
            self.electrum_wallet.create_tx(&mut create_opt),
            Err(Error::EmptyAddressees)
        ));
    }

    pub fn utxos(&self) -> Vec<UnblindedTXO> {
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
