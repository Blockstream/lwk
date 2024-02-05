extern crate lwk_wollet;

use crate::bitcoin::amount::Denomination;
use crate::bitcoin::bip32::{DerivationPath, Xpriv};
use crate::bitcoin::{Amount, Network};
use crate::elements::hashes::Hash;
use crate::elements::hex::ToHex;
use crate::elements::pset::PartiallySignedTransaction;
use crate::elements::{Address, AssetId, ContractHash, OutPoint, TxOutWitness, Txid};
use bip39::Mnemonic;
use electrsd::bitcoind::bitcoincore_rpc::{Client, RpcApi};
use electrsd::electrum_client::ElectrumApi;
use elements::confidential::{AssetBlindingFactor, ValueBlindingFactor};
use elements::encode::Decodable;
use elements::hex::FromHex;
use elements::{Block, TxOutSecrets};
use elements_miniscript::descriptor::checksum::desc_checksum;
use elements_miniscript::{DescriptorPublicKey, ForEachKey};
use lwk_common::Signer;
use lwk_jade::register_multisig::{JadeDescriptor, RegisterMultisigParams};
use lwk_signer::*;
use lwk_wollet::*;
use pulldown_cmark::{CodeBlockKind, Event, Tag};
use rand::{thread_rng, Rng};
use serde_json::Value;
use std::convert::TryInto;
use std::env;
use std::str::FromStr;
use std::sync::Once;
use std::thread;
use std::time::Duration;
use tempfile::TempDir;
use tracing::metadata::LevelFilter;

pub mod jade;

const DEFAULT_FEE_RATE: f32 = 100.0;

static TRACING_INIT: Once = Once::new();

pub const TEST_MNEMONIC: &str =
    "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
pub const TEST_MNEMONIC_XPUB: &str =
"tpubD6NzVbkrYhZ4XYa9MoLt4BiMZ4gkt2faZ4BcmKu2a9te4LDpQmvEz2L2yDERivHxFPnxXXhqDRkUNnQCpZggCyEZLBktV7VaSmwayqMJy1s";
pub const TEST_MNEMONIC_SLIP77: &str =
    "9c8e4f05c7711a98c838be228bcb84924d4570ca53f35fa1c793e58841d47023";

/// Descriptor with 11 txs on testnet
pub const TEST_DESCRIPTOR: &str = "ct(slip77(ab5824f4477b4ebb00a132adfd8eb0b7935cf24f6ac151add5d1913db374ce92),elwpkh([759db348/84'/1'/0']tpubDCRMaF33e44pcJj534LXVhFbHibPbJ5vuLhSSPFAw57kYURv4tzXFL6LSnd78bkjqdmE3USedkbpXJUPA1tdzKfuYSL7PianceqAhwL2UkA/<0;1>/*))#cch6wrnp";

pub fn liquid_block_1() -> Block {
    let raw = include_bytes!(
        "../test_data/afafbbdfc52a45e51a3b634f391f952f6bdfd14ef74b34925954b4e20d0ad639.raw"
    );
    Block::consensus_decode(&raw[..]).unwrap()
}

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

pub fn parse_code_from_markdown(markdown_input: &str, code_kind: &str) -> Vec<String> {
    let parser = pulldown_cmark::Parser::new(markdown_input);
    let mut result = vec![];
    let mut str = String::new();
    let mut active = false;

    for el in parser {
        match el {
            Event::Start(Tag::CodeBlock(CodeBlockKind::Fenced(current)))
                if code_kind == current.as_ref() =>
            {
                active = true
            }
            Event::Text(t) => {
                if active {
                    str.push_str(t.as_ref())
                }
            }
            Event::End(Tag::CodeBlock(CodeBlockKind::Fenced(current)))
                if code_kind == current.as_ref() =>
            {
                result.push(str.clone());
                str.clear();
                active = false;
            }
            _ => (),
        }
    }

    result
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
    pub fn new(electrs_exec: String, node_exec: String, enable_esplora_http: bool) -> Self {
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
        conf.http_enabled = enable_esplora_http;
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

    pub fn node_sendtoaddress(
        &self,
        address: &Address,
        satoshi: u64,
        asset: Option<AssetId>,
    ) -> Txid {
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
        Txid::from_str(r.as_str().unwrap()).unwrap()
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
    pub electrum_url: ElectrumUrl,
    db_root_dir: TempDir,
}

pub fn regtest_policy_asset() -> AssetId {
    AssetId::from_str("5ac9f65c0efcc4775e0baec4ec03abdde22473cd3cf33c0419ca290e0751b225").unwrap()
}

pub fn network_regtest() -> ElementsNetwork {
    let policy_asset = regtest_policy_asset();
    ElementsNetwork::ElementsRegtest { policy_asset }
}

pub fn new_unsupported_wallet(desc: &str, expected: Error) {
    let r: Result<WolletDescriptor, _> = add_checksum(desc).parse();

    match r {
        Ok(_) => panic!("Expected unsupported descriptor\n{}\n{:?}", desc, expected),
        Err(err) => assert_eq!(err.to_string(), expected.to_string()),
    }
}

impl TestWollet {
    pub fn new(electrs_url: &str, desc: &str) -> Self {
        let db_root_dir = TempDir::new().unwrap();
        Self::with_temp_dir(electrs_url, desc, db_root_dir)
    }

    pub fn with_temp_dir(electrs_url: &str, desc: &str, db_root_dir: TempDir) -> Self {
        let tls = false;
        let validate_domain = false;

        let network = network_regtest();
        let descriptor = add_checksum(desc);

        let desc: WolletDescriptor = descriptor.parse().unwrap();
        let mut wollet = Wollet::with_fs_persist(network, desc, &db_root_dir).unwrap();

        let electrum_url = ElectrumUrl::new(electrs_url, tls, validate_domain);

        let mut electrum_client: ElectrumClient = ElectrumClient::new(&electrum_url).unwrap();
        full_scan_with_electrum_client(&mut wollet, &mut electrum_client).unwrap();

        let mut i = 120;
        let tip = loop {
            assert!(i > 0, "1 minute without updates");
            i -= 1;
            let height = electrum_client.tip().unwrap().height;
            if height >= 101 {
                break height;
            } else {
                thread::sleep(Duration::from_millis(500));
            }
        };
        full_scan_with_electrum_client(&mut wollet, &mut electrum_client).unwrap();

        assert!(tip >= 101);

        Self {
            wollet,
            electrum_url,
            db_root_dir,
        }
    }

    pub fn db_root_dir(self) -> TempDir {
        self.db_root_dir
    }

    pub fn policy_asset(&self) -> AssetId {
        self.wollet.policy_asset()
    }

    pub fn sync(&mut self) {
        let mut electrum_client: ElectrumClient = ElectrumClient::new(&self.electrum_url).unwrap();
        full_scan_with_electrum_client(&mut self.wollet, &mut electrum_client).unwrap();
    }

    pub fn address(&self) -> Address {
        self.wollet.address(None).unwrap().address().clone()
    }

    pub fn address_result(&self, last_unused: Option<u32>) -> AddressResult {
        self.wollet.address(last_unused).unwrap()
    }

    /// Wait until tx appears in tx list (max 1 min)
    fn wait_for_tx(&mut self, txid: &Txid) {
        let mut electrum_client: ElectrumClient = ElectrumClient::new(&self.electrum_url).unwrap();
        for _ in 0..120 {
            full_scan_with_electrum_client(&mut self.wollet, &mut electrum_client).unwrap();
            let list = self.wollet.transactions().unwrap();
            if list.iter().any(|e| &e.tx.txid() == txid) {
                return;
            }
            thread::sleep(Duration::from_millis(500));
        }
        panic!("Wallet does not have {} in its list", txid);
    }

    /// asset balance in satoshi
    pub fn balance(&mut self, asset: &AssetId) -> u64 {
        let balance = self.wollet.balance().unwrap();
        *balance.get(asset).unwrap_or(&0u64)
    }

    fn balance_btc(&mut self) -> u64 {
        self.balance(&self.wollet.policy_asset())
    }

    fn get_tx_from_list(&mut self, txid: &Txid) -> WalletTx {
        let list = self.wollet.transactions().unwrap();
        for tx in list.iter() {
            if tx.height.is_some() {
                assert!(tx.timestamp.is_some());
            }
        }
        let filtered_list: Vec<_> = list
            .iter()
            .filter(|e| &e.tx.txid() == txid)
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
        assert_eq!(&tx.type_, "incoming");
        let wallet_txid = tx.tx.txid();
        assert_eq!(txid, wallet_txid);
        assert_eq!(tx.inputs.iter().filter(|o| o.is_some()).count(), 0);
        assert_eq!(tx.outputs.iter().filter(|o| o.is_some()).count(), 1);

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
        let txid = self.send(&mut pset);
        let balance_after = self.balance_btc();
        assert!(balance_before > balance_after);
        let tx = self.get_tx_from_list(&txid);
        // We only sent, so all balances are negative
        assert!(tx.balance.values().all(|v| *v < 0));
        assert_eq!(&tx.type_, "outgoing");
        assert_eq!(tx.fee, fee as u64);
        assert!(tx.inputs.iter().filter(|o| o.is_some()).count() > 0);
        assert!(tx.outputs.iter().filter(|o| o.is_some()).count() > 0);

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
        let txid = self.send(&mut pset);
        let tx = self.get_tx_from_list(&txid);
        assert_eq!(&tx.type_, "issuance");

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
        let tx = self.get_tx_from_list(&txid);
        assert_eq!(&tx.type_, "reissuance");

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
        let txid = self.send(&mut pset);
        let tx = self.get_tx_from_list(&txid);
        assert_eq!(&tx.type_, "burn");

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
        let electrum_client = ElectrumClient::new(&self.electrum_url).unwrap();
        let txid = electrum_client.broadcast(&tx).unwrap();
        self.wait_for_tx(&txid);
        txid
    }

    pub fn check_persistence(wollet: TestWollet) {
        let descriptor = wollet.wollet.descriptor().to_string();
        let expected_num_updates = wollet.wollet.updates().unwrap();
        let expected = wollet.wollet.balance().unwrap();
        let db_root_dir = wollet.db_root_dir();
        let network = network_regtest();

        for _ in 0..2 {
            let wollet =
                Wollet::with_fs_persist(network, descriptor.parse().unwrap(), &db_root_dir)
                    .unwrap();

            let balance = wollet.balance().unwrap();
            dbg!(&balance);
            assert_eq!(expected, balance);
            assert_eq!(expected_num_updates, wollet.updates().unwrap());
        }
    }
}

pub fn setup(enable_esplora_http: bool) -> TestElectrumServer {
    let electrs_exec = env::var("ELECTRS_LIQUID_EXEC").expect("set ELECTRS_LIQUID_EXEC");
    let node_exec = env::var("ELEMENTSD_EXEC").expect("set ELEMENTSD_EXEC");
    TestElectrumServer::new(electrs_exec, node_exec, enable_esplora_http)
}

pub fn init_logging() {
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
    bytes.to_hex()
}

pub fn generate_xprv() -> Xpriv {
    let mut seed = [0u8; 16];
    thread_rng().fill(&mut seed);
    Xpriv::new_master(Network::Regtest, &seed).unwrap()
}

pub fn generate_signer() -> SwSigner {
    let mnemonic = generate_mnemonic();
    SwSigner::new(&mnemonic, false).unwrap()
}

pub fn multisig_desc(signers: &[&AnySigner], threshold: usize) -> String {
    assert!(threshold <= signers.len());
    let xpubs = signers
        .iter()
        .map(|s| {
            let fingerprint = s.fingerprint().unwrap();
            let path_str = "/84h/0h/0h";
            let path = DerivationPath::from_str(&format!("m{path_str}")).unwrap();
            let xpub = s.derive_xpub(&path).unwrap();
            format!("[{fingerprint}{path_str}]{xpub}/<0;1>/*",)
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
        network: lwk_jade::Network::LocaltestLiquid,
        multisig_name: name.into(),
        descriptor: desc,
    };

    for signer in signers {
        if let AnySigner::Jade(s, _) = signer {
            s.register_multisig(params.clone()).unwrap();
        }
    }
}

fn n_issuances(details: &lwk_common::PsetDetails) -> usize {
    details.issuances.iter().filter(|e| e.is_issuance()).count()
}

fn n_reissuances(details: &lwk_common::PsetDetails) -> usize {
    details
        .issuances
        .iter()
        .filter(|e| e.is_reissuance())
        .count()
}

pub fn asset_blinding_factor_test_vector() -> AssetBlindingFactor {
    AssetBlindingFactor::from_hex(
        "0000000000000000000000000000000000000000000000000000000000000001",
    )
    .unwrap()
}

pub fn value_blinding_factor_test_vector() -> ValueBlindingFactor {
    ValueBlindingFactor::from_hex(
        "0000000000000000000000000000000000000000000000000000000000000002",
    )
    .unwrap()
}

pub fn txid_test_vector() -> Txid {
    Txid::from_str("0000000000000000000000000000000000000000000000000000000000000003").unwrap()
}

pub fn tx_out_secrets_test_vector() -> TxOutSecrets {
    elements::TxOutSecrets::new(
        regtest_policy_asset(),
        asset_blinding_factor_test_vector(),
        1000,
        value_blinding_factor_test_vector(),
    )
}

pub fn tx_out_secrets_test_vector_bytes() -> Vec<u8> {
    Vec::<u8>::from_hex(include_str!("../test_data/tx_out_secrets_test_vector.hex")).unwrap()
}

pub fn update_test_vector_bytes() -> Vec<u8> {
    Vec::<u8>::from_hex(include_str!("../test_data/update_test_vector.hex")).unwrap()
}

#[cfg(test)]
mod test {

    use crate::parse_code_from_markdown;

    #[test]
    fn test_parse_code_from_markdown() {
        let mkdown = r#"
```python
python
code
```
```rust
rust
code
```

```python
some more
python code
"#;
        let res = parse_code_from_markdown(mkdown, "python");
        assert_eq!(
            res,
            vec![
                "python\ncode\n".to_string(),
                "some more\npython code\n".to_string()
            ]
        );

        let res = parse_code_from_markdown(mkdown, "rust");
        assert_eq!(res, vec!["rust\ncode\n".to_string()])
    }
}
