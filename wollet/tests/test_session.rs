extern crate wollet;

use bip39::Mnemonic;
use chrono::Utc;
use electrsd::bitcoind::bitcoincore_rpc::{Client, RpcApi};
use electrum_client::ElectrumApi;
use elements_miniscript::descriptor::checksum::desc_checksum;
use elements_miniscript::elements::bitcoin::amount::Denomination;
use elements_miniscript::elements::bitcoin::hashes::sha256;
use elements_miniscript::elements::bitcoin::hashes::Hash;
use elements_miniscript::elements::bitcoin::{Amount, Network, PrivateKey};
use elements_miniscript::elements::hex::ToHex;
use elements_miniscript::elements::issuance::ContractHash;
use elements_miniscript::elements::pset::PartiallySignedTransaction;
use elements_miniscript::elements::{Address, AssetId, OutPoint, Transaction, TxOutWitness, Txid};
use log::{LevelFilter, Metadata, Record};
use rand::{thread_rng, Rng};
use serde_json::Value;
use software_signer::*;
use std::env;
use std::str::FromStr;
use std::sync::Once;
use std::thread;
use std::time::Duration;
use tempfile::TempDir;
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

    pub fn generate(&self, blocks: u32) {
        node_generate(&self.node.client, blocks);
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

    pub fn node_getnewaddress(&self) -> Address {
        node_getnewaddress(&self.node.client, None)
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
        let _db_root_dir = TempDir::new().unwrap();

        let db_root = format!("{}", _db_root_dir.path().display());

        let mut electrum_wallet = ElectrumWallet::new(
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
            if tip.0 >= 101 {
                break tip.0;
            } else {
                thread::sleep(Duration::from_millis(500));
            }
        };
        assert!(tip >= 101);

        Self {
            electrum_wallet,
            _db_root_dir,
        }
    }

    pub fn policy_asset(&self) -> AssetId {
        self.electrum_wallet.policy_asset()
    }

    fn address(&self) -> Address {
        self.electrum_wallet.address().unwrap().address
    }

    pub fn full_address(&self) -> AddressResult {
        self.electrum_wallet.address().unwrap()
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
    fn balance(&mut self, asset: &AssetId) -> u64 {
        self.electrum_wallet.sync_txs().unwrap();
        let balance = self.electrum_wallet.balance().unwrap();
        *balance.get(asset).unwrap_or(&0u64)
    }

    fn balance_btc(&mut self) -> u64 {
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

    pub fn fund(
        &mut self,
        server: &mut TestElectrumServer,
        satoshi: u64,
        address: Option<Address>,
        asset: Option<AssetId>,
    ) {
        let utxos_before = self.electrum_wallet.utxos().unwrap().len();
        let balance_before = self.balance(&asset.unwrap_or(self.policy_asset()));

        let address = address.unwrap_or_else(|| self.address());
        let txid = server.node_sendtoaddress(&address, satoshi, asset);
        self.wait_for_tx(&txid);
        let wallet_txid = self.get_tx_from_list(&txid).txid().to_string();
        assert_eq!(txid, wallet_txid);

        let utxos_after = self.electrum_wallet.utxos().unwrap().len();
        let balance_after = self.balance(&asset.unwrap_or(self.policy_asset()));
        assert_eq!(utxos_after, utxos_before + 1);
        assert_eq!(balance_before + satoshi, balance_after);
    }

    pub fn fund_btc(&mut self, server: &mut TestElectrumServer) {
        self.fund(server, 1_000_000, Some(self.address()), None);
    }

    pub fn fund_asset(&mut self, server: &mut TestElectrumServer) -> AssetId {
        let num_utxos_before = self.electrum_wallet.utxos().unwrap().len();
        let satoshi = 10_000;
        let address = self.address();
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

    pub fn send_btc(&mut self, signers: &[&Signer]) {
        let balance_before = self.balance_btc();
        let satoshi: u64 = 10_000;
        let address = self.address();
        let mut pset = self
            .electrum_wallet
            .sendlbtc(satoshi, &address.to_string())
            .unwrap();

        let balance = pset_balance(
            &pset,
            self.electrum_wallet.unblinded(),
            self.electrum_wallet.descriptor(),
        )
        .unwrap();
        assert_eq!(balance.fee, 1_000);
        assert_eq!(balance.balances.get(&self.policy_asset()), Some(&-1000));

        for signer in signers {
            self.sign(signer, &mut pset);
        }
        self.send(&mut pset);
        let balance_after = self.balance_btc();
        assert!(balance_before > balance_after);
    }

    pub fn send_asset(&mut self, signers: &[&Signer], node_address: &Address, asset: &AssetId) {
        let balance_before = self.balance(asset);
        let satoshi: u64 = 10;
        let mut pset = self
            .electrum_wallet
            .sendasset(satoshi, &node_address.to_string(), &asset.to_string())
            .unwrap();

        for signer in signers {
            self.sign(signer, &mut pset);
        }
        self.send(&mut pset);
        let balance_after = self.balance(asset);
        assert!(balance_before > balance_after);
    }

    pub fn send_many(
        &mut self,
        signers: &[&Signer],
        addr1: &Address,
        asset1: &AssetId,
        addr2: &Address,
        asset2: &AssetId,
    ) {
        let balance1_before = self.balance(asset1);
        let balance2_before = self.balance(asset2);
        let addr1 = addr1.to_string();
        let addr2 = addr2.to_string();
        let ass1 = asset1.to_string();
        let ass2 = asset2.to_string();
        let addressees: Vec<UnvalidatedAddressee> = vec![
            UnvalidatedAddressee {
                satoshi: 1_000,
                address: &addr1,
                asset: &ass1,
            },
            UnvalidatedAddressee {
                satoshi: 2_000,
                address: &addr2,
                asset: &ass2,
            },
        ];
        let mut pset = self.electrum_wallet.sendmany(addressees).unwrap();

        for signer in signers {
            self.sign(signer, &mut pset);
        }
        self.send(&mut pset);
        let balance1_after = self.balance(asset1);
        let balance2_after = self.balance(asset2);
        assert!(balance1_before > balance1_after);
        assert!(balance2_before > balance2_after);
    }

    pub fn issueasset(
        &mut self,
        signers: &[&Signer],
        satoshi_asset: u64,
        satoshi_token: u64,
    ) -> (AssetId, AssetId, [u8; 32]) {
        let balance_before = self.balance_btc();
        let mut pset = self
            .electrum_wallet
            .issueasset(satoshi_asset, satoshi_token)
            .unwrap();

        let input = &pset.inputs()[0];
        let asset_entropy = input.issuance_asset_entropy.unwrap();
        let contract_hash = ContractHash::from_byte_array(asset_entropy);
        let prevout = OutPoint::new(input.previous_txid, input.previous_output_index);
        let entropy = AssetId::generate_asset_entropy(prevout, contract_hash);

        for signer in signers {
            self.sign(signer, &mut pset);
        }
        self.send(&mut pset);

        let (asset, token) = pset.inputs()[0].issuance_ids();
        assert_eq!(self.balance(&asset), satoshi_asset);
        assert_eq!(self.balance(&token), satoshi_token);
        let balance_after = self.balance_btc();
        assert!(balance_before > balance_after);

        (asset, token, entropy.to_byte_array())
    }

    pub fn reissueasset(
        &mut self,
        signers: &[&Signer],
        satoshi_asset: u64,
        asset: &AssetId,
        token: &AssetId,
        entropy: &[u8; 32],
    ) {
        let entropy = sha256::Midstate::from_slice(&entropy[..]).unwrap();
        let entropy = entropy.to_string();
        let balance_btc_before = self.balance_btc();
        let balance_asset_before = self.balance(asset);
        let balance_token_before = self.balance(token);
        let mut pset = self
            .electrum_wallet
            .reissueasset(&entropy, satoshi_asset)
            .unwrap();
        for signer in signers {
            self.sign(signer, &mut pset);
        }
        self.send(&mut pset);

        assert_eq!(self.balance(asset), balance_asset_before + satoshi_asset);
        assert_eq!(self.balance(token), balance_token_before);
        assert!(self.balance_btc() < balance_btc_before);
    }

    pub fn burnasset(&mut self, signers: &[&Signer], satoshi_asset: u64, asset: &AssetId) {
        let balance_btc_before = self.balance_btc();
        let balance_asset_before = self.balance(asset);
        let mut pset = self
            .electrum_wallet
            .burnasset(&asset.to_string(), satoshi_asset)
            .unwrap();
        for signer in signers {
            self.sign(signer, &mut pset);
        }
        self.send(&mut pset);

        assert_eq!(self.balance(asset), balance_asset_before - satoshi_asset);
        assert!(self.balance_btc() < balance_btc_before);
    }

    fn sign(&self, signer: &Signer, pset: &mut PartiallySignedTransaction) {
        let pset_base64 = pset_to_base64(pset);
        let signed_pset_base64 = signer.sign(&pset_base64).unwrap();
        assert_ne!(pset_base64, signed_pset_base64);
        *pset = pset_from_base64(&signed_pset_base64).unwrap();
    }

    fn send(&mut self, pset: &mut PartiallySignedTransaction) -> Txid {
        let tx = self.electrum_wallet.finalize(pset).unwrap();
        let txid = self.electrum_wallet.broadcast(&tx).unwrap();
        self.wait_for_tx(&txid.to_string());
        txid
    }
}

pub fn setup() -> TestElectrumServer {
    let electrs_exec = env::var("ELECTRS_LIQUID_EXEC").expect("set ELECTRS_LIQUID_EXEC");
    let node_exec = env::var("ELEMENTSD_EXEC").expect("set ELEMENTSD_EXEC");
    TestElectrumServer::new(electrs_exec, node_exec)
}

#[allow(dead_code)]
pub fn prune_proofs(pset: &PartiallySignedTransaction) -> PartiallySignedTransaction {
    let mut pset = pset.clone();
    for i in pset.inputs_mut() {
        if let Some(utxo) = &mut i.witness_utxo {
            utxo.witness = TxOutWitness::default();
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

fn generate_mnemonic() -> String {
    let mut bytes = [0u8; 16];
    thread_rng().fill(&mut bytes);
    Mnemonic::from_entropy(&bytes).unwrap().to_string()
}

pub fn generate_slip77() -> String {
    let mut bytes = [0u8; 32];
    thread_rng().fill(&mut bytes);
    bytes.to_hex()
}

pub fn generate_view_key() -> String {
    let mut bytes = [0u8; 32];
    thread_rng().fill(&mut bytes);
    PrivateKey::from_slice(&bytes, Network::Regtest)
        .unwrap()
        .to_wif()
}

pub fn generate_signer() -> Signer<'static> {
    let mnemonic = generate_mnemonic();
    Signer::new(&mnemonic, &wollet::EC).unwrap()
}
