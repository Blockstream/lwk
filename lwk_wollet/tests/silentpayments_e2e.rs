#![cfg(feature = "silentpayments")]

use lwk_common::silentpayments::{SilentPaymentAccount, SilentPaymentSigner};
use lwk_common::{singlesig_desc, DescriptorBlindingKey, Signer, Singlesig};
use lwk_signer::SwSigner;
use lwk_test_util::{TestEnv, TestEnvBuilder, DEFAULT_SPECULOS_MNEMONIC, TEST_MNEMONIC};
use lwk_wollet::clients::blocking::{BlockchainBackend, EsploraClient};
use lwk_wollet::elements::bitcoin::bip32::{ChildNumber, DerivationPath, Xpriv};
use lwk_wollet::elements::{AssetId, Txid};
use lwk_wollet::silentpayments::{InputKey, MapInputProvider, SilentPaymentScanMaterial};
use lwk_wollet::{Capability, Network, Wollet, WolletBuilder, EC};

const FUNDED: u64 = 1_000_000;
const SENT: u64 = 100_000;

const STRANGER_MNEMONIC: &str = DEFAULT_SPECULOS_MNEMONIC;

/// A regtest environment with a funded sender wallet.
struct SpEnv {
    env: TestEnv,
    client: EsploraClient,
    signer: SwSigner,
    sender: Wollet,
}

impl SpEnv {
    fn new() -> Self {
        let env = TestEnvBuilder::from_env().with_esplora().build();

        // Each regtest node mines its own genesis block, and the Elements taproot
        // sighash commits to that hash. Using the default regtest parameters here
        // would make the signer commit to a genesis the chain never had, so the
        // network must come from the running node.
        let network = env.elementsd_network();
        let client = EsploraClient::new(&env.esplora_url(), network)
            .expect("the regtest esplora endpoint must be reachable");

        let signer = SwSigner::new(TEST_MNEMONIC, false).expect("the test mnemonic must be valid");
        let sender = Self::wallet_for(network, &signer, None);

        let mut this = SpEnv {
            env,
            client,
            signer,
            sender,
        };
        this.fund_sender();
        this
    }

    fn wallet_for(
        network: Network,
        signer: &SwSigner,
        material: Option<SilentPaymentScanMaterial>,
    ) -> Wollet {
        let desc: lwk_wollet::WolletDescriptor =
            singlesig_desc(signer, Singlesig::Wpkh, DescriptorBlindingKey::Slip77)
                .expect("descriptor derivation must succeed")
                .parse()
                .expect("the derived descriptor must parse");

        let mut builder = WolletBuilder::new(network, desc);
        if let Some(material) = material {
            builder = builder.with_silent_payment_material(material);
        }
        builder.build().expect("building the wallet must succeed")
    }

    fn scan_material(mnemonic: &str) -> SilentPaymentScanMaterial {
        SwSigner::new(mnemonic, false)
            .expect("the mnemonic must be valid")
            .silent_payment_scan_material(SilentPaymentAccount::liquid_testnet(0))
            .expect("a software signer must export scan material")
    }

    fn policy_asset(&self) -> AssetId {
        self.sender.policy_asset()
    }

    /// Syncs the sender, tolerating a dropped connection.
    ///
    /// Callers poll this in a loop, and several regtest nodes running at once
    /// occasionally drop an HTTP response. A transient failure is retried by the
    /// next iteration; a real one still fails the surrounding wait.
    fn sync(&mut self) {
        if let Ok(Some(update)) = self.client.full_scan(&self.sender) {
            self.sender
                .apply_update(update)
                .expect("applying the sender update must succeed");
        }
    }

    fn fund_sender(&mut self) {
        let address = self
            .sender
            .address(None)
            .expect("deriving an address must succeed")
            .address()
            .clone();
        let txid = self.env.elementsd_sendtoaddress(&address, FUNDED, None);
        self.env.elementsd_generate(2);
        self.wait_for_sender_tx(&txid);
    }

    /// Waits until the sender sees `txid` in a block, returning its height.
    ///
    /// Esplora indexes a transaction as soon as it reaches the mempool, so
    /// waiting for mere presence would race the confirmation we assert on.
    fn wait_for_sender_tx(&mut self, txid: &Txid) -> u32 {
        for _ in 0..120 {
            self.sync();
            if let Some(height) = self
                .sender
                .transaction(txid)
                .expect("looking up a transaction must succeed")
                .and_then(|tx| tx.height)
            {
                return height;
            }
            std::thread::sleep(std::time::Duration::from_millis(500));
        }
        panic!("the sender never saw {txid} confirmed");
    }

    /// Provides keys for the sender's current UTXOs.
    fn input_provider_for_utxos(&self) -> MapInputProvider {
        let master = Xpriv::new_master(
            lwk_wollet::elements::bitcoin::Network::Regtest,
            &lwk_signer::bip39::Mnemonic::parse(TEST_MNEMONIC)
                .expect("the test mnemonic must parse")
                .to_seed(""),
        )
        .expect("deriving a master key must succeed");

        let account: DerivationPath = "m/84'/1'/0'"
            .parse()
            .expect("the account path must be valid");

        self.sender
            .utxos()
            .expect("listing the sender's UTXOs must succeed")
            .into_iter()
            .map(|utxo| {
                let path = account
                    .child(
                        ChildNumber::from_normal_idx(utxo.ext_int as u32)
                            .expect("the chain index is normal"),
                    )
                    .child(
                        ChildNumber::from_normal_idx(utxo.wildcard_index)
                            .expect("the address index is normal"),
                    );
                let key = master
                    .derive_priv(&EC, &path)
                    .expect("deriving a wallet key must succeed")
                    .private_key;
                (utxo.outpoint, InputKey::Plain(key))
            })
            .collect()
    }

    /// Sends to `sp_address`, confirms, and returns the txid and height.
    fn send_silent_payment(&mut self, sp_address: &str, satoshi: u64) -> (Txid, u32) {
        let asset = self.policy_asset();
        let mut pset = self
            .sender
            .tx_builder()
            .add_silent_payment_recipient(sp_address, satoshi, asset)
            .expect("adding a silent payment recipient must succeed")
            .finish_silent_payment(&self.input_provider_for_utxos())
            .expect("building the silent payment must succeed");

        self.signer.sign(&mut pset).expect("signing must succeed");
        let tx = self
            .sender
            .finalize(&mut pset)
            .expect("the signed transaction must finalize");

        let txid = self
            .client
            .broadcast(&tx)
            .expect("broadcasting must succeed");
        self.env.elementsd_generate(2);
        let height = self.wait_for_sender_tx(&txid);

        (txid, height)
    }

    /// Builds a recipient wallet with unrelated descriptor data.
    fn recipient(&self, material: SilentPaymentScanMaterial, birthday: u32) -> Wollet {
        let signer = SwSigner::new(STRANGER_MNEMONIC, false).expect("the mnemonic must be valid");
        let desc: lwk_wollet::WolletDescriptor =
            singlesig_desc(&signer, Singlesig::Wpkh, DescriptorBlindingKey::Slip77)
                .expect("descriptor derivation must succeed")
                .parse()
                .expect("the derived descriptor must parse");

        WolletBuilder::new(self.env.elementsd_network(), desc)
            .with_silent_payment_material(material)
            .with_silent_payment_birthday(birthday)
            .build()
            .expect("building the recipient must succeed")
    }

    fn scan(&mut self, wollet: &mut Wollet) {
        if let Some(update) = self
            .client
            .full_scan(wollet)
            .expect("scanning must succeed")
        {
            wollet
                .apply_update(update)
                .expect("applying the update must succeed");
        }
    }

    fn balance(wollet: &Wollet, asset: &AssetId) -> u64 {
        wollet
            .balance()
            .expect("computing the balance must succeed")
            .get(asset)
            .copied()
            .unwrap_or(0)
    }
}

#[test]
fn regtest_esplora_advertises_silent_payments() {
    let env = TestEnvBuilder::from_env().with_esplora().build();
    let client = EsploraClient::new(&env.esplora_url(), env.elementsd_network()).unwrap();

    assert!(
        client.capabilities().contains(&Capability::SilentPayments),
        "esplora computes tweaks client-side, so it must advertise discovery"
    );
}

#[test]
fn a_silent_payment_is_sent_discovered_and_spent() {
    let mut env = SpEnv::new();
    let asset = env.policy_asset();
    let material = SpEnv::scan_material(TEST_MNEMONIC);

    let birthday = env.sender.tip().height();
    let sp_address = env
        .recipient(material, birthday)
        .silent_payment_address()
        .expect("a wallet with scan material must state where to pay it");

    let (txid, height) = env.send_silent_payment(&sp_address, SENT);

    let mut recipient = env.recipient(material, birthday);
    env.scan(&mut recipient);

    assert_eq!(
        SpEnv::balance(&recipient, &asset),
        SENT,
        "discovery must find the payment and count it in the balance"
    );

    let utxo = recipient
        .utxos()
        .expect("listing the recipient's UTXOs must succeed")
        .into_iter()
        .find(|u| u.outpoint.txid == txid)
        .expect("the discovered UTXO must come from the broadcast transaction");
    assert_eq!(utxo.unblinded.value, SENT);

    assert_eq!(
        recipient.silent_payments_scanned_to(),
        Some(env.sender.tip().height()),
        "a completed scan must record how far discovery reached"
    );
    assert!(height >= birthday);

    let change = env
        .sender
        .address(None)
        .expect("deriving an address must succeed")
        .address()
        .clone();
    let mut pset = recipient
        .tx_builder()
        .add_recipient(&change, SENT / 2, asset)
        .expect("adding a recipient must succeed")
        .finish()
        .expect("coin selection must fund the spend from the discovered payment");

    let signer = SwSigner::new(TEST_MNEMONIC, false).unwrap();
    let signed = signer
        .sign(&mut pset)
        .expect("the signer must sign the silent payment input");
    assert_eq!(signed, 1, "exactly the silent payment input is signed");

    let tx = recipient
        .finalize(&mut pset)
        .expect("the signed spend must finalize");
    let spend_txid = env
        .client
        .broadcast(&tx)
        .expect("the network must accept the spend of a silent payment output");
    env.env.elementsd_generate(2);

    env.scan(&mut recipient);
    assert!(
        SpEnv::balance(&recipient, &asset) < SENT,
        "spending the discovered payment must reduce the balance"
    );
    assert!(
        recipient
            .transaction(&spend_txid)
            .expect("looking up the spend must succeed")
            .is_some(),
        "the wallet must see its own spend confirmed"
    );
}

#[test]
fn a_stranger_does_not_discover_the_payment() {
    let mut env = SpEnv::new();
    let asset = env.policy_asset();
    let material = SpEnv::scan_material(TEST_MNEMONIC);

    let birthday = env.sender.tip().height();
    let sp_address = env
        .recipient(material, birthday)
        .silent_payment_address()
        .expect("a wallet with scan material must state where to pay it");

    env.send_silent_payment(&sp_address, SENT);

    let stranger_material = SpEnv::scan_material(STRANGER_MNEMONIC);
    let mut stranger = env.recipient(stranger_material, birthday);
    env.scan(&mut stranger);

    assert_eq!(
        SpEnv::balance(&stranger, &asset),
        0,
        "different scan material must not discover this payment"
    );
}

#[test]
fn full_scan_without_scan_material_claims_no_discovery() {
    let mut env = SpEnv::new();

    let update = env
        .client
        .full_scan(&env.sender)
        .expect("a full scan must succeed");

    if let Some(update) = update {
        assert!(
            update.silent_payments.is_none(),
            "no scan material means no discovery claim, so adding it later rescans history"
        );
    }
}
