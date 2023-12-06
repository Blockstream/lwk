extern crate wollet;

use crate::bitcoin::amount::Denomination;
use crate::bitcoin::bip32::ExtendedPrivKey;
use crate::bitcoin::{Amount, Network, PrivateKey};
use crate::elements::hashes::Hash;
use crate::elements::hex::ToHex;
use crate::elements::pset::PartiallySignedTransaction;
use crate::elements::{Address, AssetId, ContractHash, OutPoint, TxOutWitness, Txid};
use bip39::Mnemonic;
use common::Signer;
use electrsd::bitcoind::bitcoincore_rpc::{Client, RpcApi};
use electrum_client::ElectrumApi;
use elements_miniscript::descriptor::checksum::desc_checksum;
use elements_miniscript::{DescriptorPublicKey, ForEachKey};
use jade::register_multisig::{JadeDescriptor, RegisterMultisigParams};
use rand::{thread_rng, Rng};
use serde_json::Value;
use signer::*;
use std::convert::TryInto;
use std::env;
use std::str::FromStr;
use std::sync::Once;
use std::thread;
use std::time::Duration;
use tempfile::TempDir;
use tracing::metadata::LevelFilter;
use wollet::*;

const DEFAULT_FEE_RATE: f32 = 100.0;

static TRACING_INIT: Once = Once::new();

fn add_checksum(desc: &str) -> String {
    if desc.find('#').is_some() {
        desc.into()
    } else {
        format!("{}#{}", desc, desc_checksum(desc).unwrap())
    }
}

fn compute_fee_rate(pset: &PartiallySignedTransaction) -> f32 {
    let vsize = pset.extract_tx().unwrap().vsize();
    let fee_satoshi = pset.outputs().last().unwrap().amount.unwrap();
    1000.0 * (fee_satoshi as f32 / vsize as f32)
}

fn assert_fee_rate(fee_rate: f32, expected: Option<f32>) {
    let expected = expected.unwrap_or(DEFAULT_FEE_RATE);
    let toll = 0.05;
    assert!(fee_rate > expected * (1.0 - toll));
    assert!(fee_rate < expected * (1.0 + toll));
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

/// Serialize and deserialize a PSET
///
/// This allows us to catch early (de)serialization issues,
/// which can be hit in practice since PSETs are passed around as b64 strings.
fn pset_rt(pset: &PartiallySignedTransaction) -> PartiallySignedTransaction {
    PartiallySignedTransaction::from_str(&pset.to_string()).unwrap()
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

        let node = electrsd::bitcoind::BitcoinD::with_conf(node_exec, &conf).unwrap();

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
        let electrs = electrsd::ElectrsD::with_conf(electrs_exec, &node, &conf).unwrap();

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

    pub fn node_issueasset(&self, satoshi: u64) -> AssetId {
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
}

pub struct TestWollet {
    pub wollet: Wollet,
    _db_root_dir: TempDir,
}

fn network_regtest() -> ElementsNetwork {
    let policy_asset =
        AssetId::from_str("5ac9f65c0efcc4775e0baec4ec03abdde22473cd3cf33c0419ca290e0751b225")
            .unwrap();
    ElementsNetwork::ElementsRegtest { policy_asset }
}

pub fn new_unsupported_wallet(desc: &str, expected: Error) {
    let r = Wollet::new(
        network_regtest(),
        "",
        false,
        false,
        "/tmp",
        &add_checksum(desc),
    );
    match r {
        Ok(_) => panic!("Expected unsupported descriptor\n{}\n{:?}", desc, expected),
        Err(err) => assert_eq!(err.to_string(), expected.to_string()),
    }
}

impl TestWollet {
    pub fn new(electrs_url: &str, desc: &str) -> Self {
        let tls = false;
        let validate_domain = false;
        let _db_root_dir = TempDir::new().unwrap();

        let db_root = format!("{}", _db_root_dir.path().display());

        let mut wollet = Wollet::new(
            network_regtest(),
            electrs_url,
            tls,
            validate_domain,
            &db_root,
            &add_checksum(desc),
        )
        .unwrap();

        wollet.sync_txs().unwrap();
        let list = wollet.transactions().unwrap();
        assert_eq!(list.len(), 0);
        let mut i = 120;
        let tip = loop {
            assert!(i > 0, "1 minute without updates");
            i -= 1;
            wollet.sync_tip().unwrap();
            let tip = wollet.tip().unwrap();
            if tip.0 >= 101 {
                break tip.0;
            } else {
                thread::sleep(Duration::from_millis(500));
            }
        };
        assert!(tip >= 101);

        Self {
            wollet,
            _db_root_dir,
        }
    }

    pub fn policy_asset(&self) -> AssetId {
        self.wollet.policy_asset()
    }

    pub fn sync(&mut self) {
        self.wollet.sync_txs().unwrap();
        self.wollet.sync_tip().unwrap();
    }

    pub fn address(&self) -> Address {
        self.wollet.address(None).unwrap().address().clone()
    }

    pub fn address_result(&self, last_unused: Option<u32>) -> AddressResult {
        self.wollet.address(last_unused).unwrap()
    }

    /// Wait until tx appears in tx list (max 1 min)
    fn wait_for_tx(&mut self, txid: &str) {
        for _ in 0..120 {
            self.wollet.sync_txs().unwrap();
            let list = self.wollet.transactions().unwrap();
            if list.iter().any(|e| e.tx.txid().to_string() == txid) {
                return;
            }
            thread::sleep(Duration::from_millis(500));
        }
        panic!("Wallet does not have {} in its list", txid);
    }

    /// asset balance in satoshi
    pub fn balance(&mut self, asset: &AssetId) -> u64 {
        self.wollet.sync_txs().unwrap();
        let balance = self.wollet.balance().unwrap();
        *balance.get(asset).unwrap_or(&0u64)
    }

    fn balance_btc(&mut self) -> u64 {
        self.balance(&self.wollet.policy_asset())
    }

    fn get_tx_from_list(&mut self, txid: &str) -> WalletTx {
        self.wollet.sync_txs().unwrap();
        let list = self.wollet.transactions().unwrap();
        let filtered_list: Vec<_> = list
            .iter()
            .filter(|e| e.tx.txid().to_string() == txid)
            .cloned()
            .collect();
        assert!(
            !filtered_list.is_empty(),
            "just made tx {} is not in tx list",
            txid
        );
        filtered_list.first().unwrap().clone()
    }

    pub fn fund(
        &mut self,
        server: &TestElectrumServer,
        satoshi: u64,
        address: Option<Address>,
        asset: Option<AssetId>,
    ) {
        let utxos_before = self.wollet.utxos().unwrap().len();
        let balance_before = self.balance(&asset.unwrap_or(self.policy_asset()));

        let address = address.unwrap_or_else(|| self.address());
        let txid = server.node_sendtoaddress(&address, satoshi, asset);
        self.wait_for_tx(&txid);
        let tx = self.get_tx_from_list(&txid);
        // We only received, all balances are positive
        assert!(tx.balance.values().all(|v| *v > 0));
        let wallet_txid = tx.tx.txid().to_string();
        assert_eq!(txid, wallet_txid);

        let utxos_after = self.wollet.utxos().unwrap().len();
        let balance_after = self.balance(&asset.unwrap_or(self.policy_asset()));
        assert_eq!(utxos_after, utxos_before + 1);
        assert_eq!(balance_before + satoshi, balance_after);
    }

    pub fn fund_btc(&mut self, server: &TestElectrumServer) {
        self.fund(server, 1_000_000, Some(self.address()), None);
    }

    pub fn fund_asset(&mut self, server: &TestElectrumServer) -> AssetId {
        let satoshi = 10_000;
        let asset = server.node_issueasset(satoshi);
        self.fund(server, satoshi, Some(self.address()), Some(asset));
        asset
    }

    /// Send 10_000 satoshi to self with default fee rate.
    ///
    /// To specify a custom fee rate pass Some in `fee_rate`
    /// To specify an external recipient specify the `to` parameter
    pub fn send_btc(
        &mut self,
        signers: &[&AnySigner],
        fee_rate: Option<f32>,
        external: Option<(Address, u64)>,
    ) {
        let balance_before = self.balance_btc();

        let recipient = external.clone().unwrap_or((self.address(), 10_000));

        let mut pset = self
            .wollet
            .send_lbtc(recipient.1, &recipient.0.to_string(), fee_rate)
            .unwrap();
        pset = pset_rt(&pset);

        let details = self.wollet.get_details(&pset).unwrap();
        let fee = details.balance.fee as i64;
        assert!(fee > 0);
        let balance = match &external {
            Some((_a, v)) => -fee - *v as i64,
            None => -fee,
        };
        assert_eq!(
            *details.balance.balances.get(&self.policy_asset()).unwrap(),
            balance
        );
        assert_eq!(n_issuances(&details), 0);
        assert_eq!(n_reissuances(&details), 0);

        for signer in signers {
            self.sign(signer, &mut pset);
        }
        assert_fee_rate(compute_fee_rate(&pset), fee_rate);
        let txid = self.send(&mut pset).to_string();
        let balance_after = self.balance_btc();
        assert!(balance_before > balance_after);
        let tx = self.get_tx_from_list(&txid);
        // We only sent, so all balances are negative
        assert!(tx.balance.values().all(|v| *v < 0));
        assert_eq!(tx.fee, fee as u64);

        self.wollet.descriptor().descriptor.for_each_key(|k| {
            if let DescriptorPublicKey::XPub(x) = k {
                if let Some(origin) = &x.origin {
                    assert_eq!(pset.global.xpub.get(&x.xkey).unwrap(), origin);
                }
            }
            true
        });
    }

    pub fn send_asset(
        &mut self,
        signers: &[&AnySigner],
        node_address: &Address,
        asset: &AssetId,
        fee_rate: Option<f32>,
    ) {
        let balance_before = self.balance(asset);
        let satoshi: u64 = 10;
        let mut pset = self
            .wollet
            .send_asset(
                satoshi,
                &node_address.to_string(),
                &asset.to_string(),
                fee_rate,
            )
            .unwrap();
        pset = pset_rt(&pset);

        let details = self.wollet.get_details(&pset).unwrap();
        let fee = details.balance.fee as i64;
        assert!(fee > 0);
        assert_eq!(
            *details.balance.balances.get(&self.policy_asset()).unwrap(),
            -fee
        );
        assert_eq!(
            *details.balance.balances.get(asset).unwrap(),
            -(satoshi as i64)
        );
        assert_eq!(n_issuances(&details), 0);
        assert_eq!(n_reissuances(&details), 0);

        for signer in signers {
            self.sign(signer, &mut pset);
        }
        assert_fee_rate(compute_fee_rate(&pset), fee_rate);
        self.send(&mut pset);
        let balance_after = self.balance(asset);
        assert!(balance_before > balance_after);
    }

    pub fn send_many(
        &mut self,
        signers: &[&AnySigner],
        addr1: &Address,
        asset1: &AssetId,
        addr2: &Address,
        asset2: &AssetId,
        fee_rate: Option<f32>,
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
                address: addr1,
                asset: ass1,
            },
            UnvalidatedAddressee {
                satoshi: 2_000,
                address: addr2,
                asset: ass2,
            },
        ];
        let mut pset = self.wollet.send_many(addressees, fee_rate).unwrap();
        pset = pset_rt(&pset);

        let details = self.wollet.get_details(&pset).unwrap();
        let fee = details.balance.fee as i64;
        assert!(fee > 0);
        // Checking the balance here has a bit too many cases:
        // asset1,2 are btc, asset1,2 are equal, addr1,2 belong to the wallet
        // Skipping the checks here
        assert_eq!(n_issuances(&details), 0);
        assert_eq!(n_reissuances(&details), 0);

        for signer in signers {
            self.sign(signer, &mut pset);
        }
        assert_fee_rate(compute_fee_rate(&pset), fee_rate);
        self.send(&mut pset);
        let balance1_after = self.balance(asset1);
        let balance2_after = self.balance(asset2);
        assert!(balance1_before > balance1_after);
        assert!(balance2_before > balance2_after);
    }

    pub fn issueasset(
        &mut self,
        signers: &[&AnySigner],
        satoshi_asset: u64,
        satoshi_token: u64,
        contract: &str,
        fee_rate: Option<f32>,
    ) -> (AssetId, AssetId) {
        let balance_before = self.balance_btc();
        let mut pset = self
            .wollet
            .issue_asset(satoshi_asset, "", satoshi_token, "", contract, fee_rate)
            .unwrap();
        pset = pset_rt(&pset);

        let issuance_input = &pset.inputs()[0].clone();
        let (asset, token) = issuance_input.issuance_ids();

        let details = self.wollet.get_details(&pset).unwrap();
        assert_eq!(n_issuances(&details), 1);
        assert_eq!(n_reissuances(&details), 0);
        let issuance = &details.issuances[0];
        assert_eq!(asset, issuance.asset().unwrap());
        assert_eq!(token, issuance.token().unwrap());
        assert_eq!(satoshi_asset, issuance.asset_satoshi().unwrap());
        assert_eq!(satoshi_token, issuance.token_satoshi().unwrap());
        let fee = details.balance.fee as i64;
        assert!(fee > 0);
        assert_eq!(
            *details.balance.balances.get(&self.policy_asset()).unwrap(),
            -fee
        );
        assert_eq!(
            *details.balance.balances.get(&asset).unwrap(),
            satoshi_asset as i64
        );
        assert_eq!(
            *details.balance.balances.get(&token).unwrap_or(&0),
            satoshi_token as i64
        );

        for signer in signers {
            self.sign(signer, &mut pset);
        }
        assert_fee_rate(compute_fee_rate(&pset), fee_rate);
        self.send(&mut pset);

        assert_eq!(self.balance(&asset), satoshi_asset);
        assert_eq!(self.balance(&token), satoshi_token);
        let balance_after = self.balance_btc();
        assert!(balance_before > balance_after);

        let issuance = self.wollet.issuance(&asset).unwrap();
        assert_eq!(issuance.vin, 0);
        assert!(!issuance.is_reissuance);
        assert_eq!(issuance.asset_amount, Some(satoshi_asset));
        assert_eq!(issuance.token_amount, Some(satoshi_token));

        let prevout = OutPoint::new(
            issuance_input.previous_txid,
            issuance_input.previous_output_index,
        );
        let contract_hash = if contract.is_empty() {
            ContractHash::from_slice(&[0u8; 32]).unwrap()
        } else {
            ContractHash::from_json_contract(contract).unwrap()
        };
        assert_eq!(asset, AssetId::new_issuance(prevout, contract_hash));

        (asset, token)
    }

    pub fn reissueasset(
        &mut self,
        signers: &[&AnySigner],
        satoshi_asset: u64,
        asset: &AssetId,
        fee_rate: Option<f32>,
    ) {
        let issuance = self.wollet.issuance(asset).unwrap();
        let balance_btc_before = self.balance_btc();
        let balance_asset_before = self.balance(asset);
        let balance_token_before = self.balance(&issuance.token);
        let mut pset = self
            .wollet
            .reissue_asset(asset.to_string().as_str(), satoshi_asset, "", fee_rate)
            .unwrap();
        pset = pset_rt(&pset);

        let details = self.wollet.get_details(&pset).unwrap();
        assert_eq!(n_issuances(&details), 0);
        assert_eq!(n_reissuances(&details), 1);
        let reissuance = details
            .issuances
            .iter()
            .find(|e| e.is_reissuance())
            .unwrap();
        assert_eq!(asset, &reissuance.asset().unwrap());
        assert_eq!(issuance.token, reissuance.token().unwrap());
        assert_eq!(satoshi_asset, reissuance.asset_satoshi().unwrap());
        assert!(reissuance.token_satoshi().is_none());
        let fee = details.balance.fee as i64;
        assert!(fee > 0);
        assert_eq!(
            *details.balance.balances.get(&self.policy_asset()).unwrap(),
            -fee
        );
        assert_eq!(
            *details.balance.balances.get(asset).unwrap(),
            satoshi_asset as i64
        );
        assert_eq!(
            *details.balance.balances.get(&issuance.token).unwrap(),
            0i64
        );

        for signer in signers {
            self.sign(signer, &mut pset);
        }
        assert_fee_rate(compute_fee_rate(&pset), fee_rate);
        let txid = self.send(&mut pset);

        assert_eq!(self.balance(asset), balance_asset_before + satoshi_asset);
        assert_eq!(self.balance(&issuance.token), balance_token_before);
        assert!(self.balance_btc() < balance_btc_before);

        let issuances = self.wollet.issuances().unwrap();
        assert!(issuances.len() > 1);
        let reissuance = issuances.iter().find(|e| e.txid == txid).unwrap();
        assert!(reissuance.is_reissuance);
        assert_eq!(reissuance.asset_amount, Some(satoshi_asset));
        assert!(reissuance.token_amount.is_none());
    }

    pub fn burnasset(
        &mut self,
        signers: &[&AnySigner],
        satoshi_asset: u64,
        asset: &AssetId,
        fee_rate: Option<f32>,
    ) {
        let balance_btc_before = self.balance_btc();
        let balance_asset_before = self.balance(asset);
        let mut pset = self
            .wollet
            .burn_asset(&asset.to_string(), satoshi_asset, fee_rate)
            .unwrap();
        pset = pset_rt(&pset);

        let details = self.wollet.get_details(&pset).unwrap();
        let fee = details.balance.fee as i64;
        assert!(fee > 0);
        let btc = self.policy_asset();
        let (expected_asset, expected_btc) = if asset == &btc {
            (0, -(fee + satoshi_asset as i64))
        } else {
            (-(satoshi_asset as i64), -fee)
        };
        assert_eq!(*details.balance.balances.get(&btc).unwrap(), expected_btc);
        assert_eq!(
            *details.balance.balances.get(asset).unwrap_or(&0),
            expected_asset
        );
        assert_eq!(n_issuances(&details), 0);
        assert_eq!(n_reissuances(&details), 0);

        for signer in signers {
            self.sign(signer, &mut pset);
        }
        assert_fee_rate(compute_fee_rate(&pset), fee_rate);
        self.send(&mut pset);

        assert_eq!(self.balance(asset), balance_asset_before - satoshi_asset);
        assert!(self.balance_btc() < balance_btc_before);
    }

    pub fn sign<S: Signer>(&self, signer: &S, pset: &mut PartiallySignedTransaction) {
        *pset = pset_rt(pset);
        let sigs_added_or_overwritten = signer.sign(pset).unwrap();
        assert!(sigs_added_or_overwritten > 0);
    }

    pub fn send(&mut self, pset: &mut PartiallySignedTransaction) -> Txid {
        *pset = pset_rt(pset);
        let tx = self.wollet.finalize(pset).unwrap();
        let txid = self.wollet.broadcast(&tx).unwrap();
        self.wait_for_tx(&txid.to_string());
        txid
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

pub fn generate_xprv() -> ExtendedPrivKey {
    let mut seed = [0u8; 16];
    thread_rng().fill(&mut seed);
    ExtendedPrivKey::new_master(Network::Regtest, &seed).unwrap()
}

pub fn generate_signer() -> SwSigner {
    let mnemonic = generate_mnemonic();
    SwSigner::new(&mnemonic).unwrap()
}

pub fn multisig_desc(signers: &[&AnySigner], threshold: usize) -> String {
    assert!(threshold <= signers.len());
    let xpubs = signers
        .iter()
        .map(|s| {
            let fingerprint = s.fingerprint().unwrap();
            let xpub = s.xpub().unwrap();
            format!("[{fingerprint}]{xpub}/<0;1>/*",)
        })
        .collect::<Vec<_>>()
        .join(",");
    let slip77 = generate_slip77();
    format!("ct(slip77({slip77}),elwsh(multi({threshold},{xpubs})))")
}

pub fn register_multisig(signers: &[&AnySigner], name: &str, desc: &str) {
    // Register a multisig descriptor on each *jade* signer
    let desc: WolletDescriptor = desc.parse().unwrap();
    let desc: JadeDescriptor = desc.as_ref().try_into().unwrap();
    let params = RegisterMultisigParams {
        network: jade::Network::LocaltestLiquid,
        multisig_name: name.into(),
        descriptor: desc,
    };

    for signer in signers {
        if let AnySigner::Jade(s, _) = signer {
            s.register_multisig(params.clone());
        }
    }
}

fn n_issuances(details: &common::PsetDetails) -> usize {
    details.issuances.iter().filter(|e| e.is_issuance()).count()
}

fn n_reissuances(details: &common::PsetDetails) -> usize {
    details
        .issuances
        .iter()
        .filter(|e| e.is_reissuance())
        .count()
}
