use crate::amp2::Amp2D;
use crate::auth::AuthStack;
use crate::init_logging;
use crate::registry::RegistryD;
use crate::waterfalls::WaterfallsD;

use electrsd::bitcoind;
use electrsd::electrum_client::ElectrumApi;
use electrsd::ElectrsD;

use bitcoind::bitcoincore_rpc::{Client, RpcApi};
use bitcoind::BitcoinD;

use elements::bitcoin;
use elements_miniscript::elements;

use elements::hex::FromHex;
use elements::{Address, AssetId, BlockHash, Txid};

use lwk_common::Network;
use serde_json::Value;
use std::net::TcpListener;
use std::str::FromStr;
use std::time::Duration;

/// Configure and start the test environment
pub struct TestEnvBuilder {
    elementsd_exec: String,
    electrs_exec: String,
    bitcoind_exec: String,
    waterfalls_exec: String,
    registry_exec: String,
    amp2_exec: String,
    with_electrum: bool,
    with_esplora: bool,
    with_bitcoind: bool,
    with_waterfalls: bool,
    waterfalls_max_txs_seen: Option<usize>,
    with_registry: bool,
    with_amp2: bool,
    with_zmq: bool,
    with_auth: bool,
}

impl TestEnvBuilder {
    /// Create TestEnvBuilder reading from environment variables
    ///
    /// * ELEMENTSD_EXEC
    /// * ELECTRS_LIQUID_EXEC
    /// * BITCOIND_EXEC
    /// * WATERFALLS_EXEC
    /// * ASSET_REGISTRY_EXEC
    /// * AMP2_MOCK_EXEC
    pub fn from_env() -> Self {
        let elementsd_exec = std::env::var("ELEMENTSD_EXEC").unwrap_or_default();
        let electrs_exec = std::env::var("ELECTRS_LIQUID_EXEC").unwrap_or_default();
        let bitcoind_exec = std::env::var("BITCOIND_EXEC").unwrap_or_default();
        let waterfalls_exec = std::env::var("WATERFALLS_EXEC").unwrap_or_default();
        let registry_exec = std::env::var("ASSET_REGISTRY_EXEC").unwrap_or_default();
        let amp2_exec = std::env::var("AMP2_MOCK_EXEC").unwrap_or_default();

        Self {
            elementsd_exec,
            electrs_exec,
            bitcoind_exec,
            waterfalls_exec,
            registry_exec,
            amp2_exec,
            with_electrum: false,
            with_esplora: false,
            with_bitcoind: false,
            with_waterfalls: false,
            waterfalls_max_txs_seen: None,
            with_registry: false,
            with_amp2: false,
            with_zmq: false,
            with_auth: false,
        }
    }

    /// Start an Electrum server
    pub fn with_electrum(mut self) -> Self {
        self.with_electrum = true;
        self
    }

    /// Start an Esplora server
    pub fn with_esplora(mut self) -> Self {
        self.with_esplora = true;
        self
    }

    /// Start a Bitcoin node
    pub fn with_bitcoind(mut self) -> Self {
        self.with_bitcoind = true;
        self
    }

    /// Start a Waterfalls server
    pub fn with_waterfalls(mut self) -> Self {
        self.with_waterfalls = true;
        self
    }

    /// Start a Waterfalls server with a custom per-script history truncation limit
    pub fn with_waterfalls_max_txs_seen(mut self, max_txs_seen: usize) -> Self {
        self.with_waterfalls = true;
        self.waterfalls_max_txs_seen = Some(max_txs_seen);
        self
    }

    /// Start a Asset Registry server
    pub fn with_registry(mut self) -> Self {
        self.with_registry = true;
        self
    }

    /// Start an AMP2 mock server
    pub fn with_amp2(mut self) -> Self {
        self.with_amp2 = true;
        self
    }

    /// Start elementsd with ZMQ endpoints
    pub fn with_zmq(mut self) -> Self {
        self.with_zmq = true;
        self
    }

    /// Start an authenticated gateway (Keycloak + redis + APISIX) fronting the Esplora
    /// server, or the Waterfalls server when `with_waterfalls()` is also set
    ///
    /// Requires `with_esplora()` and docker.
    pub fn with_auth(mut self) -> Self {
        self.with_auth = true;
        self
    }

    /// Start the test environment
    pub fn build(self) -> TestEnv {
        if self.elementsd_exec.is_empty() {
            panic!("ELEMENTSD_EXEC must be set");
        }
        if self.electrs_exec.is_empty() && (self.with_electrum || self.with_esplora) {
            panic!("ELECTRS_LIQUID_EXEC must be set");
        }
        if self.bitcoind_exec.is_empty() && self.with_bitcoind {
            panic!("BITCOIND_EXEC must be set");
        }
        if self.waterfalls_exec.is_empty() && self.with_waterfalls {
            panic!("WATERFALLS_EXEC must be set");
        }
        if self.with_registry {
            if self.registry_exec.is_empty() {
                panic!("ASSET_REGISTRY_EXEC must be set");
            }
            if !self.with_esplora {
                panic!("asset registry requires esplora, call 'with_esplora()'");
            }
        }
        if self.amp2_exec.is_empty() && self.with_amp2 {
            panic!("AMP2_MOCK_EXEC must be set");
        }
        if self.with_auth && !self.with_esplora && !self.with_waterfalls {
            panic!("auth gateway requires esplora or waterfalls, call 'with_esplora()' or 'with_waterfalls()'");
        }
        if self.with_auth && self.with_esplora && self.with_waterfalls {
            panic!("auth gateway fronts either esplora or waterfalls, not both (yet): enable only one of them with 'with_auth()'");
        }

        init_logging();

        let bitcoind = if self.with_bitcoind {
            Some(BitcoinD::new(self.bitcoind_exec).unwrap())
        } else {
            None
        };

        // Start elementsd
        let view_stdout = std::env::var("RUST_LOG").is_ok();

        //TODO remove this bad code once Conf::args is not Vec<&str>
        fn string_to_static_str(s: String) -> &'static str {
            Box::leak(s.into_boxed_str())
        }

        let mut args = vec![
            "-fallbackfee=0.0001",
            "-dustrelayfee=0.00000001",
            "-chain=liquidregtest",
            "-initialfreecoins=2100000000",
            "-acceptdiscountct=1",
            "-multi_data_permitted=1",
            "-rest",
            "-txindex=1",
            "-evbparams=simplicity:-1:::", // Enable Simplicity from block 0
            "-minrelaytxfee=0",            // test tx with no fees/asset fees
            "-blockmintxfee=0",            // test tx with no fees/asset fees
            "-acceptunlimitedissuances=1",
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

        let zmq_endpoint = if self.with_zmq {
            let addr = TcpListener::bind("0.0.0.0:0")
                .unwrap()
                .local_addr()
                .unwrap()
                .to_string();
            let endpoint = format!("tcp://{addr}");

            args.push(string_to_static_str(format!("-zmqpubrawtx={endpoint}")));
            args.push(string_to_static_str(format!("-zmqpubrawblock={endpoint}")));
            args.push(string_to_static_str(format!("-zmqpubhashtx={endpoint}")));
            args.push(string_to_static_str(format!("-zmqpubhashblock={endpoint}")));
            args.push(string_to_static_str(format!("-zmqpubsequence={endpoint}")));

            Some(endpoint)
        } else {
            None
        };

        let network = "liquidregtest";

        let mut elements_conf = bitcoind::Conf::default();
        elements_conf.args = args;
        elements_conf.view_stdout = view_stdout;
        elements_conf.p2p = bitcoind::P2P::Yes;
        elements_conf.network = network;

        let elementsd = BitcoinD::with_conf(&self.elementsd_exec, &elements_conf).unwrap();

        TestEnv::elementsd_generate_(&elementsd.client, 1);
        TestEnv::rescanblockchain_(&elementsd.client);
        TestEnv::elementsd_sweep_initialfreecoins_(&elementsd.client);
        TestEnv::elementsd_generate_(&elementsd.client, 100);

        // Start electrs
        let electrsd = if self.with_electrum || self.with_esplora {
            let args = if view_stdout { vec!["-v"] } else { vec![] };
            let mut electrs_conf = electrsd::Conf::default();
            electrs_conf.args = args;
            electrs_conf.view_stderr = view_stdout;
            electrs_conf.http_enabled = self.with_esplora;
            electrs_conf.network = network;
            let electrsd =
                ElectrsD::with_conf(&self.electrs_exec, &elementsd, &electrs_conf).unwrap();

            electrsd.trigger().unwrap();

            let mut i = 120;
            loop {
                assert!(i > 0, "1 minute without updates");
                i -= 1;
                let height = electrsd
                    .client
                    .block_headers_subscribe_raw()
                    .unwrap()
                    .height;
                if height == 101 {
                    break;
                }
                std::thread::sleep(Duration::from_millis(500));
            }
            Some(electrsd)
        } else {
            None
        };

        let waterfallsd = if self.with_waterfalls {
            let rpc = elementsd.rpc_url();
            let cookie_values = elementsd.params.get_cookie_values().unwrap().unwrap();
            let waterfallsd = WaterfallsD::new(
                &self.waterfalls_exec,
                &rpc,
                &cookie_values.user,
                &cookie_values.password,
                self.waterfalls_max_txs_seen,
            );
            Some(waterfallsd)
        } else {
            None
        };

        let registryd = if self.with_registry {
            let esplora_url = electrsd.as_ref().unwrap().esplora_url.as_ref().unwrap();
            Some(RegistryD::new(
                &self.registry_exec,
                &format!("http://{esplora_url}"),
            ))
        } else {
            None
        };

        let amp2d = if self.with_amp2 {
            Some(Amp2D::new(&self.amp2_exec))
        } else {
            None
        };

        let auth = if self.with_auth {
            // The gateway fronts the Waterfalls server when it runs (its API is a superset
            // used by the waterfalls clients), the plain Esplora server otherwise.
            let upstream_url = match waterfallsd.as_ref() {
                Some(waterfallsd) => waterfallsd.waterfalls_url().to_string(),
                None => electrsd
                    .as_ref()
                    .unwrap()
                    .esplora_url
                    .as_ref()
                    .unwrap()
                    .to_string(),
            };
            let upstream_port: u16 = upstream_url
                .rsplit(':')
                .next()
                .and_then(|p| p.parse().ok())
                .unwrap_or_else(|| panic!("cannot parse upstream port from '{upstream_url}'"));
            Some(AuthStack::new(upstream_port))
        } else {
            None
        };

        TestEnv {
            elementsd,
            bitcoind,
            electrsd,
            waterfallsd,
            registryd,
            amp2d,
            zmq_endpoint,
            auth,
        }
    }
}

/// Test environment with regtest Liquid node and servers
///
/// Use `TestEnvBuilder` to configure and build
pub struct TestEnv {
    elementsd: BitcoinD,
    bitcoind: Option<BitcoinD>,
    electrsd: Option<ElectrsD>,
    waterfallsd: Option<WaterfallsD>,
    registryd: Option<RegistryD>,
    amp2d: Option<Amp2D>,
    zmq_endpoint: Option<String>,
    auth: Option<AuthStack>,
}

impl TestEnv {
    pub fn zmq_endpoint(&self) -> String {
        self.zmq_endpoint.as_ref().unwrap().clone()
    }

    pub fn electrum_url(&self) -> String {
        let url = &self.electrsd.as_ref().unwrap().electrum_url;
        format!("tcp://{url}")
    }

    pub fn esplora_url(&self) -> String {
        let url = &self
            .electrsd
            .as_ref()
            .unwrap()
            .esplora_url
            .as_ref()
            .unwrap();
        format!("http://{url}")
    }

    pub fn waterfalls_url(&self) -> String {
        self.waterfallsd
            .as_ref()
            .unwrap()
            .waterfalls_url()
            .to_string()
    }

    pub fn registry_url(&self) -> String {
        self.registryd
            .as_ref()
            .map(|r| r.url().to_string())
            .unwrap_or_default()
    }

    pub fn amp2_url(&self) -> String {
        self.amp2d.as_ref().unwrap().url().to_string()
    }

    /// Base url of the authenticated gateway fronting the Esplora server (requires `with_auth`)
    pub fn esplora_gateway_url(&self) -> String {
        self.gateway_url()
    }

    /// Base url of the authenticated gateway (requires `with_auth`); it fronts the Waterfalls
    /// server when `with_waterfalls` is set, the Esplora server otherwise
    pub fn gateway_url(&self) -> String {
        self.auth.as_ref().unwrap().gateway_url()
    }

    /// The OAuth2 token endpoint of the auth gateway (requires `with_auth`)
    pub fn oidc_token_url(&self) -> String {
        self.auth.as_ref().unwrap().token_url()
    }

    /// Set the credit balance of the test user on the auth gateway (requires `with_auth`)
    pub fn set_credits(&self, credits: u64) {
        self.auth.as_ref().unwrap().set_credits(credits)
    }

    // Functions for Elements RPC client

    pub fn elements_rpc_url(&self) -> String {
        self.elementsd.rpc_url()
    }

    pub fn elements_rpc_credentials(&self) -> (String, String) {
        let cookie_values = self.elementsd.params.get_cookie_values().unwrap().unwrap();
        (cookie_values.user, cookie_values.password)
    }

    // Elementsd methods

    // Generic Elements RPC
    pub fn elementsd_call(&self, cmd: &str, args: &[serde_json::Value]) -> serde_json::Value {
        self.elementsd.client.call(cmd, args).unwrap()
    }

    fn rescanblockchain_(client: &Client) {
        client.call::<Value>("rescanblockchain", &[]).unwrap();
    }

    fn elementsd_getnewaddress_(client: &Client) -> Address {
        let kind = "p2sh-segwit";
        let addr: Value = client
            .call("getnewaddress", &["label".into(), kind.into()])
            .unwrap();
        Address::from_str(addr.as_str().unwrap()).unwrap()
    }

    pub fn elementsd_getnewaddress(&self) -> Address {
        Self::elementsd_getnewaddress_(&self.elementsd.client)
    }

    fn elementsd_generate_(client: &Client, block_num: u32) {
        let address = Self::elementsd_getnewaddress_(client).to_string();
        client
            .call::<Value>("generatetoaddress", &[block_num.into(), address.into()])
            .unwrap();
    }

    pub fn elementsd_generate(&self, blocks: u32) {
        Self::elementsd_generate_(&self.elementsd.client, blocks);

        // After we generate blocks, trigger an electrs update
        if let Some(electrsd) = &self.electrsd {
            electrsd.trigger().unwrap();
        }
    }

    fn elementsd_sweep_initialfreecoins_(client: &Client) {
        let address = Self::elementsd_getnewaddress_(client);
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
    }

    pub fn elementsd_sendtoaddress(
        &self,
        address: &Address,
        satoshi: u64,
        asset: Option<AssetId>,
    ) -> Txid {
        let btc = sat2btc(satoshi);
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
        let btc = sat2btc(satoshi);
        let r = self
            .elementsd
            .client
            .call::<Value>("issueasset", &[btc.into(), 0.into()])
            .unwrap();
        let asset = r.get("asset").unwrap().as_str().unwrap().to_string();
        AssetId::from_str(&asset).unwrap()
    }

    pub fn elementsd_height(&self) -> u64 {
        let raw: serde_json::Value = self
            .elementsd
            .client
            .call("getblockchaininfo", &[])
            .unwrap();
        raw.get("blocks").unwrap().as_u64().unwrap()
    }

    /// Get the genesis block hash from the running elementsd node.
    ///
    /// Could differ from the hardcoded one because parameters like `-initialfreecoins`
    /// change the genesis hash.
    pub fn elementsd_genesis_block_hash(&self) -> BlockHash {
        let raw: Value = self
            .elementsd
            .client
            .call("getblockhash", &[0.into()])
            .unwrap();
        BlockHash::from_str(raw.as_str().unwrap()).unwrap()
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

    /// Get the policy_asset from the running elementsd node.
    pub fn elementsd_policy_asset(&self) -> AssetId {
        let value: serde_json::Value = self.elementsd.client.call("getsidechaininfo", &[]).unwrap();
        AssetId::from_str(value.get("pegged_asset").unwrap().as_str().unwrap()).unwrap()
    }

    /// Get the network from the running elementsd node.
    pub fn elementsd_network(&self) -> Network {
        let genesis_hash = self.elementsd_genesis_block_hash();
        let policy_asset = self.elementsd_policy_asset();

        Network::CustomElements(
            lwk_common::ElementsParamsBuilder::new()
                .with_genesis_hash(genesis_hash)
                .with_policy_asset(policy_asset)
                .build()
                .expect("static"),
        )
    }

    // methods on bitcoind

    fn bitcoind_getnewaddress_(client: &Client) -> bitcoin::Address {
        let kind = "p2sh-segwit";
        let addr: Value = client
            .call("getnewaddress", &["label".into(), kind.into()])
            .unwrap();
        bitcoin::Address::from_str(addr.as_str().unwrap())
            .unwrap()
            .assume_checked()
    }

    fn bitcoind_generate_(client: &Client, block_num: u32) {
        let address = Self::bitcoind_getnewaddress_(client).to_string();
        client
            .call::<Value>("generatetoaddress", &[block_num.into(), address.into()])
            .unwrap();
    }

    pub fn bitcoind(&self) -> &electrsd::bitcoind::BitcoinD {
        self.bitcoind.as_ref().unwrap()
    }

    pub fn bitcoind_generate(&self, blocks: u32) {
        Self::bitcoind_generate_(&self.bitcoind().client, blocks)
    }

    pub fn bitcoind_sendtoaddress(
        &self,
        address: &bitcoin::Address,
        satoshi: u64,
    ) -> bitcoin::Txid {
        let btc = sat2btc(satoshi);
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
}

fn sat2btc(sat: u64) -> String {
    let amount = bitcoin::Amount::from_sat(sat);
    amount.to_string_in(bitcoin::amount::Denomination::Bitcoin)
}
