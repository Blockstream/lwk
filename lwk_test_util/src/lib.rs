use electrsd::bitcoind::BitcoinD;
use elements_miniscript::elements::{self, BlockHeader};

use electrsd::bitcoind::bitcoincore_rpc::{Client, RpcApi};
use electrsd::electrum_client::ElectrumApi;
use elements::bitcoin::amount::Denomination;
use elements::bitcoin::bip32::Xpriv;
use elements::bitcoin::{self, Amount, Network};
use elements::confidential::{AssetBlindingFactor, ValueBlindingFactor};
use elements::encode::Decodable;
use elements::hex::{FromHex, ToHex};
use elements::pset::PartiallySignedTransaction;
use elements::{Address, AssetId, TxOutWitness, Txid};
use elements::{Block, TxOutSecrets};
use elements_miniscript::descriptor::checksum::desc_checksum;
use pulldown_cmark::{CodeBlockKind, Event, Tag};
use rand::{thread_rng, Rng};
use serde_json::Value;
use std::env;
use std::str::FromStr;
use std::thread;
use std::time::Duration;

mod test_env;
mod waterfalls;
pub use test_env::{TestEnv, TestEnvBuilder};

const DEFAULT_FEE_RATE: f32 = 100.0;

pub const TEST_MNEMONIC: &str =
    "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
pub const TEST_MNEMONIC_XPUB: &str =
"tpubD6NzVbkrYhZ4XYa9MoLt4BiMZ4gkt2faZ4BcmKu2a9te4LDpQmvEz2L2yDERivHxFPnxXXhqDRkUNnQCpZggCyEZLBktV7VaSmwayqMJy1s";
pub const TEST_MNEMONIC_SLIP77: &str =
    "9c8e4f05c7711a98c838be228bcb84924d4570ca53f35fa1c793e58841d47023";

pub const DEFAULT_SPECULOS_MNEMONIC: &str = "glory promote mansion idle axis finger extra february uncover one trip resource lawn turtle enact monster seven myth punch hobby comfort wild raise skin";

// Constants for pegins
// test vector created with:
// ```
// $ elements-cli getnetworkinfo | jq .version
// 230201
// $ elements-cli getblockchaininfo | jq .blocks
// 2976078
// elements-cli getsidechaininfo | jq '.current_fedpegscripts[0]'`
// "5b21020e0338c96a8870479f2396c373cc7696ba124e8635d41b0ea581112b678172612102675333a4e4b8fb51d9d4e22fa5a8eaced3fdac8a8cbf9be8c030f75712e6af992102896807d54bc55c24981f24a453c60ad3e8993d693732288068a23df3d9f50d4821029e51a5ef5db3137051de8323b001749932f2ff0d34c82e96a2c2461de96ae56c2102a4e1a9638d46923272c266631d94d36bdb03a64ee0e14c7518e49d2f29bc401021031c41fdbcebe17bec8d49816e00ca1b5ac34766b91c9f2ac37d39c63e5e008afb2103079e252e85abffd3c401a69b087e590a9b86f33f574f08129ccbd3521ecf516b2103111cf405b627e22135b3b3733a4a34aa5723fb0f58379a16d32861bf576b0ec2210318f331b3e5d38156da6633b31929c5b220349859cc9ca3d33fb4e68aa08401742103230dae6b4ac93480aeab26d000841298e3b8f6157028e47b0897c1e025165de121035abff4281ff00660f99ab27bb53e6b33689c2cd8dcd364bc3c90ca5aea0d71a62103bd45cddfacf2083b14310ae4a84e25de61e451637346325222747b157446614c2103cc297026b06c71cbfa52089149157b5ff23de027ac5ab781800a578192d175462103d3bde5d63bdb3a6379b461be64dad45eabff42f758543a9645afd42f6d4248282103ed1e8d5109c9ed66f7941bc53cc71137baa76d50d274bda8d5e8ffbd6e61fe9a5fae736402c00fb269522103aab896d53a8e7d6433137bbba940f9c521e085dd07e60994579b64a6d992cf79210291b7d0b1b692f8f524516ed950872e5da10fb1b808b5a526dedc6fed1cf29807210386aa9372fbab374593466bc5451dc59954e90787f08060964d95c87ef34ca5bb53ae68"
// $ elements-cli getpeginaddress
// {
// "mainchain_address": "bc1qyya0twwz58kgfslpdgsygeq0r4nngl9tkt89v6phk8nqrwyenwrq5h0dk8",
// "claim_script": "0014a15906e643f2c9635527ab8658d370e8eaf149b5"
// }
// ```
pub const FED_PEG_SCRIPT: &str = "5b21020e0338c96a8870479f2396c373cc7696ba124e8635d41b0ea581112b678172612102675333a4e4b8fb51d9d4e22fa5a8eaced3fdac8a8cbf9be8c030f75712e6af992102896807d54bc55c24981f24a453c60ad3e8993d693732288068a23df3d9f50d4821029e51a5ef5db3137051de8323b001749932f2ff0d34c82e96a2c2461de96ae56c2102a4e1a9638d46923272c266631d94d36bdb03a64ee0e14c7518e49d2f29bc401021031c41fdbcebe17bec8d49816e00ca1b5ac34766b91c9f2ac37d39c63e5e008afb2103079e252e85abffd3c401a69b087e590a9b86f33f574f08129ccbd3521ecf516b2103111cf405b627e22135b3b3733a4a34aa5723fb0f58379a16d32861bf576b0ec2210318f331b3e5d38156da6633b31929c5b220349859cc9ca3d33fb4e68aa08401742103230dae6b4ac93480aeab26d000841298e3b8f6157028e47b0897c1e025165de121035abff4281ff00660f99ab27bb53e6b33689c2cd8dcd364bc3c90ca5aea0d71a62103bd45cddfacf2083b14310ae4a84e25de61e451637346325222747b157446614c2103cc297026b06c71cbfa52089149157b5ff23de027ac5ab781800a578192d175462103d3bde5d63bdb3a6379b461be64dad45eabff42f758543a9645afd42f6d4248282103ed1e8d5109c9ed66f7941bc53cc71137baa76d50d274bda8d5e8ffbd6e61fe9a5fae736402c00fb269522103aab896d53a8e7d6433137bbba940f9c521e085dd07e60994579b64a6d992cf79210291b7d0b1b692f8f524516ed950872e5da10fb1b808b5a526dedc6fed1cf29807210386aa9372fbab374593466bc5451dc59954e90787f08060964d95c87ef34ca5bb53ae68";
pub const FED_PEG_SCRIPT_ASM: &str = "OP_PUSHNUM_11 OP_PUSHBYTES_33 020e0338c96a8870479f2396c373cc7696ba124e8635d41b0ea581112b67817261 OP_PUSHBYTES_33 02675333a4e4b8fb51d9d4e22fa5a8eaced3fdac8a8cbf9be8c030f75712e6af99 OP_PUSHBYTES_33 02896807d54bc55c24981f24a453c60ad3e8993d693732288068a23df3d9f50d48 OP_PUSHBYTES_33 029e51a5ef5db3137051de8323b001749932f2ff0d34c82e96a2c2461de96ae56c OP_PUSHBYTES_33 02a4e1a9638d46923272c266631d94d36bdb03a64ee0e14c7518e49d2f29bc4010 OP_PUSHBYTES_33 031c41fdbcebe17bec8d49816e00ca1b5ac34766b91c9f2ac37d39c63e5e008afb OP_PUSHBYTES_33 03079e252e85abffd3c401a69b087e590a9b86f33f574f08129ccbd3521ecf516b OP_PUSHBYTES_33 03111cf405b627e22135b3b3733a4a34aa5723fb0f58379a16d32861bf576b0ec2 OP_PUSHBYTES_33 0318f331b3e5d38156da6633b31929c5b220349859cc9ca3d33fb4e68aa0840174 OP_PUSHBYTES_33 03230dae6b4ac93480aeab26d000841298e3b8f6157028e47b0897c1e025165de1 OP_PUSHBYTES_33 035abff4281ff00660f99ab27bb53e6b33689c2cd8dcd364bc3c90ca5aea0d71a6 OP_PUSHBYTES_33 03bd45cddfacf2083b14310ae4a84e25de61e451637346325222747b157446614c OP_PUSHBYTES_33 03cc297026b06c71cbfa52089149157b5ff23de027ac5ab781800a578192d17546 OP_PUSHBYTES_33 03d3bde5d63bdb3a6379b461be64dad45eabff42f758543a9645afd42f6d424828 OP_PUSHBYTES_33 03ed1e8d5109c9ed66f7941bc53cc71137baa76d50d274bda8d5e8ffbd6e61fe9a OP_PUSHNUM_15 OP_CHECKMULTISIG OP_IFDUP OP_NOTIF OP_PUSHBYTES_2 c00f OP_CSV OP_VERIFY OP_PUSHNUM_2 OP_PUSHBYTES_33 03aab896d53a8e7d6433137bbba940f9c521e085dd07e60994579b64a6d992cf79 OP_PUSHBYTES_33 0291b7d0b1b692f8f524516ed950872e5da10fb1b808b5a526dedc6fed1cf29807 OP_PUSHBYTES_33 0386aa9372fbab374593466bc5451dc59954e90787f08060964d95c87ef34ca5bb OP_PUSHNUM_3 OP_CHECKMULTISIG OP_ENDIF";
pub const FED_PEG_DESC: &str = "wsh(or_d(multi(11,020e0338c96a8870479f2396c373cc7696ba124e8635d41b0ea581112b67817261,02675333a4e4b8fb51d9d4e22fa5a8eaced3fdac8a8cbf9be8c030f75712e6af99,02896807d54bc55c24981f24a453c60ad3e8993d693732288068a23df3d9f50d48,029e51a5ef5db3137051de8323b001749932f2ff0d34c82e96a2c2461de96ae56c,02a4e1a9638d46923272c266631d94d36bdb03a64ee0e14c7518e49d2f29bc4010,031c41fdbcebe17bec8d49816e00ca1b5ac34766b91c9f2ac37d39c63e5e008afb,03079e252e85abffd3c401a69b087e590a9b86f33f574f08129ccbd3521ecf516b,03111cf405b627e22135b3b3733a4a34aa5723fb0f58379a16d32861bf576b0ec2,0318f331b3e5d38156da6633b31929c5b220349859cc9ca3d33fb4e68aa0840174,03230dae6b4ac93480aeab26d000841298e3b8f6157028e47b0897c1e025165de1,035abff4281ff00660f99ab27bb53e6b33689c2cd8dcd364bc3c90ca5aea0d71a6,03bd45cddfacf2083b14310ae4a84e25de61e451637346325222747b157446614c,03cc297026b06c71cbfa52089149157b5ff23de027ac5ab781800a578192d17546,03d3bde5d63bdb3a6379b461be64dad45eabff42f758543a9645afd42f6d424828,03ed1e8d5109c9ed66f7941bc53cc71137baa76d50d274bda8d5e8ffbd6e61fe9a),and_v(v:older(4032),multi(2,03aab896d53a8e7d6433137bbba940f9c521e085dd07e60994579b64a6d992cf79,0291b7d0b1b692f8f524516ed950872e5da10fb1b808b5a526dedc6fed1cf29807,0386aa9372fbab374593466bc5451dc59954e90787f08060964d95c87ef34ca5bb))))#7jwwklk4";

pub const PEGIN_TEST_DESC: &str = "ct(slip77(ab5824f4477b4ebb00a132adfd8eb0b7935cf24f6ac151add5d1913db374ce92),elwpkh([759db348/84'/1'/0']tpubDCRMaF33e44pcJj534LXVhFbHibPbJ5vuLhSSPFAw57kYURv4tzXFL6LSnd78bkjqdmE3USedkbpXJUPA1tdzKfuYSL7PianceqAhwL2UkA/<0;1>/*))";
pub const PEGIN_TEST_ADDR: &str = "tb1qqkq6czql4zqwsylgrfzttjrn5wjeqmwfq5yn80p39amxtnkng9lsyjwm6v"; // tweak_index = 0

/// Descriptor with 11 txs on testnet
pub const TEST_DESCRIPTOR: &str = "ct(slip77(ab5824f4477b4ebb00a132adfd8eb0b7935cf24f6ac151add5d1913db374ce92),elwpkh([759db348/84'/1'/0']tpubDCRMaF33e44pcJj534LXVhFbHibPbJ5vuLhSSPFAw57kYURv4tzXFL6LSnd78bkjqdmE3USedkbpXJUPA1tdzKfuYSL7PianceqAhwL2UkA/<0;1>/*))#cch6wrnp";

pub fn liquid_block_1() -> Block {
    let raw = include_bytes!(
        "../test_data/afafbbdfc52a45e51a3b634f391f952f6bdfd14ef74b34925954b4e20d0ad639.raw"
    );
    Block::consensus_decode(&raw[..]).unwrap()
}

pub fn liquid_block_header_2_963_520() -> BlockHeader {
    let hex = include_str!("../test_data/block_header_2_963_520.hex");
    let bytes = Vec::<u8>::from_hex(hex).unwrap();
    BlockHeader::consensus_decode(&bytes[..]).unwrap()
}

pub fn add_checksum(desc: &str) -> String {
    if desc.find('#').is_some() {
        desc.into()
    } else {
        format!("{}#{}", desc, desc_checksum(desc).unwrap())
    }
}

pub fn compute_fee_rate_without_discount_ct(pset: &PartiallySignedTransaction) -> f32 {
    let vsize = pset.extract_tx().unwrap().vsize();
    let fee_satoshi = pset.outputs().last().unwrap().amount.unwrap();
    1000.0 * (fee_satoshi as f32 / vsize as f32)
}

pub fn compute_fee_rate(pset: &PartiallySignedTransaction) -> f32 {
    let vsize = pset.extract_tx().unwrap().discount_vsize();
    let fee_satoshi = pset.outputs().last().unwrap().amount.unwrap();
    1000.0 * (fee_satoshi as f32 / vsize as f32)
}

pub fn assert_fee_rate(fee_rate: f32, expected: Option<f32>) {
    let expected = expected.unwrap_or(DEFAULT_FEE_RATE);
    let toll = 0.45;
    assert!(fee_rate > expected * (1.0 - toll));
    assert!(fee_rate < expected * (1.0 + toll));
}

fn elementsd_getnewaddress(client: &Client, kind: Option<&str>) -> Address {
    let kind = kind.unwrap_or("p2sh-segwit");
    let addr: Value = client
        .call("getnewaddress", &["label".into(), kind.into()])
        .unwrap();
    Address::from_str(addr.as_str().unwrap()).unwrap()
}

fn elementsd_generate(client: &Client, block_num: u32) {
    let address = elementsd_getnewaddress(client, None).to_string();
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
pub fn pset_rt(pset: &PartiallySignedTransaction) -> PartiallySignedTransaction {
    PartiallySignedTransaction::from_str(&pset.to_string()).unwrap()
}

pub struct TestElectrumServer {
    elementsd: BitcoinD,
    pub electrs: electrsd::ElectrsD,

    bitcoind: Option<BitcoinD>,
}

impl TestElectrumServer {
    pub fn new(
        electrs_exec: String,
        elementsd_exec: String,
        enable_esplora_http: bool,
        bitcoind_exec: Option<String>,
    ) -> Self {
        init_logging();

        let bitcoind = bitcoind_exec
            .map(|bitcoind_exec| electrsd::bitcoind::BitcoinD::new(bitcoind_exec).unwrap());

        let view_stdout = std::env::var("RUST_LOG").is_ok();

        let mut args = vec![
            "-fallbackfee=0.0001",
            "-dustrelayfee=0.00000001",
            "-chain=liquidregtest",
            "-initialfreecoins=2100000000",
            "-acceptdiscountct=1",
        ];
        if let Some(bitcoind) = bitcoind.as_ref() {
            args.push("-validatepegin=1");

            args.push(string_to_static_str(format!(
                "-mainchainrpccookiefile={}",
                bitcoind.params.cookie_file.display()
            )));
            args.push(string_to_static_str(format!(
                "-mainchainrpchost={}",
                bitcoind.params.rpc_socket.ip()
            )));
            args.push(string_to_static_str(format!(
                "-mainchainrpcport={}",
                bitcoind.params.rpc_socket.port()
            )));
        } else {
            args.push("-validatepegin=0");
        };

        let network = "liquidregtest";

        let mut conf = electrsd::bitcoind::Conf::default();
        conf.args = args;
        conf.view_stdout = view_stdout;
        conf.p2p = electrsd::bitcoind::P2P::Yes;
        conf.network = network;

        let node = electrsd::bitcoind::BitcoinD::with_conf(elementsd_exec, &conf).unwrap();

        elementsd_generate(&node.client, 1);
        node.client.call::<Value>("rescanblockchain", &[]).unwrap();
        // send initialfreecoins to the node wallet
        let address = elementsd_getnewaddress(&node.client, None);
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

        elementsd_generate(&node.client, 100);
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

        Self {
            elementsd: node,
            electrs,
            bitcoind,
        }
    }

    // methods on elementsd

    pub fn elementsd_generate(&self, blocks: u32) {
        elementsd_generate(&self.elementsd.client, blocks);
    }

    pub fn elementsd_sendtoaddress(
        &self,
        address: &Address,
        satoshi: u64,
        asset: Option<AssetId>,
    ) -> Txid {
        let amount = Amount::from_sat(satoshi);
        let btc = amount.to_string_in(Denomination::Bitcoin);
        let r = match asset {
            Some(asset) => self
                .elementsd
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
                .elementsd
                .client
                .call::<Value>("sendtoaddress", &[address.to_string().into(), btc.into()])
                .unwrap(),
        };
        Txid::from_str(r.as_str().unwrap()).unwrap()
    }

    pub fn elementsd_issueasset(&self, satoshi: u64) -> AssetId {
        let amount = Amount::from_sat(satoshi);
        let btc = amount.to_string_in(Denomination::Bitcoin);
        let r = self
            .elementsd
            .client
            .call::<Value>("issueasset", &[btc.into(), 0.into()])
            .unwrap();
        let asset = r.get("asset").unwrap().as_str().unwrap().to_string();
        AssetId::from_str(&asset).unwrap()
    }

    pub fn elementsd_getnewaddress(&self) -> Address {
        elementsd_getnewaddress(&self.elementsd.client, None)
    }

    pub fn elementsd_height(&self) -> u64 {
        let raw: serde_json::Value = self
            .elementsd
            .client
            .call("getblockchaininfo", &[])
            .unwrap();
        raw.get("blocks").unwrap().as_u64().unwrap()
    }

    pub fn elementsd_getpeginaddress(&self) -> (bitcoin::Address, String) {
        let value: serde_json::Value = self.elementsd.client.call("getpeginaddress", &[]).unwrap();

        let mainchain_address = value.get("mainchain_address").unwrap();
        let mainchain_address = bitcoin::Address::from_str(mainchain_address.as_str().unwrap())
            .unwrap()
            .assume_checked();
        let claim_script = value.get("claim_script").unwrap();
        let claim_script = claim_script.as_str().unwrap().to_string();

        (mainchain_address, claim_script)
    }

    pub fn elementsd_raw_createpsbt(&self, inputs: Value, outputs: Value) -> String {
        let psbt: serde_json::Value = self
            .elementsd
            .client
            .call("createpsbt", &[inputs, outputs, 0.into(), false.into()])
            .unwrap();
        psbt.as_str().unwrap().to_string()
    }

    pub fn elementsd_expected_next(&self, base64: &str) -> String {
        let value: serde_json::Value = self
            .elementsd
            .client
            .call("analyzepsbt", &[base64.into()])
            .unwrap();
        value.get("next").unwrap().as_str().unwrap().to_string()
    }

    pub fn elementsd_walletprocesspsbt(&self, psbt: &str) -> String {
        let value: serde_json::Value = self
            .elementsd
            .client
            .call("walletprocesspsbt", &[psbt.into()])
            .unwrap();
        value.get("psbt").unwrap().as_str().unwrap().to_string()
    }

    pub fn elementsd_finalizepsbt(&self, psbt: &str) -> String {
        let value: serde_json::Value = self
            .elementsd
            .client
            .call("finalizepsbt", &[psbt.into()])
            .unwrap();
        assert!(value.get("complete").unwrap().as_bool().unwrap());
        value.get("hex").unwrap().as_str().unwrap().to_string()
    }

    pub fn elementsd_sendrawtransaction(&self, tx: &str) -> String {
        let value: serde_json::Value = self
            .elementsd
            .client
            .call("sendrawtransaction", &[tx.into()])
            .unwrap();
        value.as_str().unwrap().to_string()
    }

    pub fn elementsd_testmempoolaccept(&self, tx: &str) -> bool {
        let value: serde_json::Value = self
            .elementsd
            .client
            .call("testmempoolaccept", &[[tx].into()])
            .unwrap();
        value.as_array().unwrap()[0]
            .get("allowed")
            .unwrap()
            .as_bool()
            .unwrap()
    }

    // methods on bitcoind

    pub fn bitcoind(&self) -> &electrsd::bitcoind::BitcoinD {
        self.bitcoind.as_ref().unwrap()
    }

    pub fn bitcoind_generate(&self, blocks: u32) {
        bitcoind_generate(&self.bitcoind().client, blocks)
    }

    pub fn bitcoind_sendtoaddress(
        &self,
        address: &bitcoin::Address,
        satoshi: u64,
    ) -> bitcoin::Txid {
        let amount = Amount::from_sat(satoshi);
        let btc = amount.to_string_in(Denomination::Bitcoin);
        let r = self
            .bitcoind()
            .client
            .call::<Value>("sendtoaddress", &[address.to_string().into(), btc.into()])
            .unwrap();
        bitcoin::Txid::from_str(r.as_str().unwrap()).unwrap()
    }

    pub fn bitcoind_getrawtransaction(&self, txid: bitcoin::Txid) -> bitcoin::Transaction {
        let r = self
            .bitcoind()
            .client
            .call::<Value>("getrawtransaction", &[txid.to_string().into()])
            .unwrap();
        let hex = r.as_str().unwrap();
        let bytes = Vec::<u8>::from_hex(hex).unwrap();
        bitcoin::consensus::deserialize(&bytes[..]).unwrap()
    }

    pub fn bitcoind_gettxoutproof(&self, txid: bitcoin::Txid) -> String {
        let arr = vec![txid.to_string()];
        let r = self
            .bitcoind()
            .client
            .call::<Value>("gettxoutproof", &[arr.into()])
            .unwrap();
        r.as_str().unwrap().to_string()
    }

    // Functions for Elements RPC client

    pub fn elements_rpc_url(&self) -> String {
        self.elementsd.rpc_url()
    }

    pub fn elements_rpc_credentials(&self) -> (String, String) {
        let cookie_values = self.elementsd.params.get_cookie_values().unwrap().unwrap();
        (cookie_values.user, cookie_values.password)
    }
}

pub fn regtest_policy_asset() -> AssetId {
    AssetId::from_str("5ac9f65c0efcc4775e0baec4ec03abdde22473cd3cf33c0419ca290e0751b225").unwrap()
}

pub fn setup() -> TestElectrumServer {
    inner_setup(false, false)
}

pub fn setup_with_esplora() -> TestElectrumServer {
    inner_setup(true, false)
}

pub fn setup_with_bitcoind() -> TestElectrumServer {
    inner_setup(false, true)
}

fn inner_setup(enable_esplora_http: bool, validate_pegin: bool) -> TestElectrumServer {
    let electrs_exec = env::var("ELECTRS_LIQUID_EXEC").expect("set ELECTRS_LIQUID_EXEC");
    let elementsd_exec = env::var("ELEMENTSD_EXEC").expect("set ELEMENTSD_EXEC");
    let bitcoind_exec = if validate_pegin {
        Some(env::var("BITCOIND_EXEC").expect("set BITCOIND_EXEC"))
    } else {
        None
    };
    TestElectrumServer::new(
        electrs_exec,
        elementsd_exec,
        enable_esplora_http,
        bitcoind_exec,
    )
}

pub fn init_logging() {
    let _ = env_logger::try_init();
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

pub fn generate_mnemonic() -> String {
    let mut bytes = [0u8; 16];
    thread_rng().fill(&mut bytes);
    bip39::Mnemonic::from_entropy(&bytes).unwrap().to_string()
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

pub fn n_issuances(details: &lwk_common::PsetDetails) -> usize {
    details.issuances.iter().filter(|e| e.is_issuance()).count()
}

pub fn n_reissuances(details: &lwk_common::PsetDetails) -> usize {
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

pub fn update_test_vector_v1_bytes() -> Vec<u8> {
    Vec::<u8>::from_hex(include_str!("../test_data/update_test_vector_v1.hex")).unwrap()
}

pub fn update_test_vector_2_bytes() -> Vec<u8> {
    include_bytes!("../test_data/update_test_vector.bin").to_vec()
}

/// An update (serialized v1) with 63 transactions on liquid testnet wallet defined by [`wollet_descriptor_many_transactions`]
pub fn update_test_vector_many_transactions() -> Vec<u8> {
    include_bytes!("../test_data/update_many_txs.bin").to_vec()
}

/// An update (serialized v2) after [`update_test_vector_many_transactions`]
pub fn update_v2_test_vector_after_many_transactions() -> Vec<u8> {
    include_bytes!("../test_data/update_v2_after_many_txs.bin").to_vec()
}

pub fn update_test_vector_encrypted_bytes() -> Vec<u8> {
    Vec::<u8>::from_hex(include_str!(
        "../test_data/update_test_vector_encrypted.hex"
    ))
    .unwrap()
}

pub fn update_test_vector_encrypted_base64() -> String {
    include_str!("../test_data/update_test_vector/update.base64").to_string()
}

pub fn update_test_vector_encrypted_bytes2() -> Vec<u8> {
    include_bytes!("../test_data/update_test_vector/000000000000").to_vec()
}

pub fn wollet_descriptor_string2() -> String {
    include_str!("../test_data/update_test_vector/desc").to_string()
}

pub fn wollet_descriptor_string() -> String {
    include_str!("../test_data/update_test_vector/desc2").to_string()
}

pub fn wollet_descriptor_many_transactions() -> &'static str {
    "ct(slip77(ac53739ddde9fdf6bba3dbc51e989b09aa8c9cdce7b7d7eddd49cec86ddf71f7),elwpkh([93970d14/84'/1'/0']tpubDC3BrFCCjXq4jAceV8k6UACxDDJCFb1eb7R7BiKYUGZdNagEhNfJoYtUrRdci9JFs1meiGGModvmNm8PrqkrEjJ6mpt6gA1DRNU8vu7GqXH/<0;1>/*))#u0y4axgs"
}

/// A 3 of 5 descriptor and a vector of partially signed transactions to combine 1 sig each
pub fn psets_to_combine() -> (String, Vec<PartiallySignedTransaction>) {
    let c = |s: &str| PartiallySignedTransaction::from_str(s).unwrap();
    let ps = vec![
        c(include_str!("../test_data/pset_combine/s1_pset.base64")),
        c(include_str!("../test_data/pset_combine/s2_pset.base64")),
        c(include_str!("../test_data/pset_combine/s3_pset.base64")),
        c(include_str!("../test_data/pset_combine/s4_pset.base64")),
        c(include_str!("../test_data/pset_combine/s5_pset.base64")),
    ];
    let d = include_str!("../test_data/pset_combine/desc");
    (d.to_string(), ps)
}

//TODO remove this bad code once Conf::args is not Vec<&str>
fn string_to_static_str(s: String) -> &'static str {
    Box::leak(s.into_boxed_str())
}

pub fn descriptor_pset_usdt_no_contracts() -> &'static str {
    include_str!("../test_data/pset_usdt/desc")
}

/// Pset created with descriptor [`descriptor_pset_usdt_no_contracts`] containing mainnet USDt but without contract info
pub fn pset_usdt_no_contracts() -> &'static str {
    include_str!("../test_data/pset_usdt/pset_usdt_no_contracts.base64")
}

pub fn pset_usdt_with_contract() -> &'static str {
    include_str!("../test_data/pset_usdt/pset_usdt_with_contract.base64")
}

fn bitcoind_getnewaddress(client: &Client, kind: Option<&str>) -> bitcoin::Address {
    let kind = kind.unwrap_or("p2sh-segwit");
    let addr: Value = client
        .call("getnewaddress", &["label".into(), kind.into()])
        .unwrap();
    bitcoin::Address::from_str(addr.as_str().unwrap())
        .unwrap()
        .assume_checked()
}

fn bitcoind_generate(client: &Client, block_num: u32) {
    let address = bitcoind_getnewaddress(client, None).to_string();
    client
        .call::<Value>("generatetoaddress", &[block_num.into(), address.into()])
        .unwrap();
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
