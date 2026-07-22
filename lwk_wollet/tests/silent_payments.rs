//! Silent payments on a real Liquid transaction: a sender builds and signs a confidential
//! transaction paying to a silent payment address, the receiver scans it and can unblind and
//! spend what it received.

use elements::bitcoin::PublicKey as BitcoinPublicKey;
use elements::confidential::{Asset, AssetBlindingFactor, Value, ValueBlindingFactor};
use elements::encode::serialize;
use elements::hashes::Hash;
use elements::secp256k1_zkp::{Message, PublicKey, SecretKey};
use elements::sighash::SighashCache;
use elements::{
    Address, AddressParams, AssetId, EcdsaSighashType, LockTime, OutPoint, Script, Sequence,
    Transaction, TxIn, TxInWitness, TxOut, TxOutSecrets, Txid,
};
use lwk_wollet::silent_payments::{
    derive_outputs, transaction_inputs, SilentPaymentInput, SilentPaymentKeys, SilentPaymentWollet,
};
use lwk_wollet::{Chain, Network, WolletDescriptor, EC};
use std::str::FromStr;

const MNEMONIC: &str =
    "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
const OTHER_MNEMONIC: &str =
    "legal winner thank year wave sausage worth useful legal winner thank yellow";
const FUNDED: u64 = 100_000;
const SENT: u64 = 90_000;
const FEE: u64 = 10_000;

/// The wallet of the sender: a single p2wpkh output it can spend
struct Sender {
    secret_key: SecretKey,
    outpoint: OutPoint,
    script_pubkey: Script,
    secrets: TxOutSecrets,
}

impl Sender {
    fn new(asset: AssetId) -> Self {
        let secret_key = SecretKey::from_slice(&[11u8; 32]).unwrap();
        let public_key = BitcoinPublicKey::new(PublicKey::from_secret_key(&EC, &secret_key));
        let address = Address::p2wpkh(&public_key, None, &AddressParams::ELEMENTS);
        Self {
            secret_key,
            outpoint: OutPoint::new(Txid::from_slice(&[7u8; 32]).unwrap(), 0),
            script_pubkey: address.script_pubkey(),
            // the output being spent is explicit, so its blinding factors are zero
            secrets: TxOutSecrets::new(
                asset,
                AssetBlindingFactor::zero(),
                FUNDED,
                ValueBlindingFactor::zero(),
            ),
        }
    }

    /// The input as the sender has it while building the transaction, before signing
    fn input(&self) -> SilentPaymentInput {
        let public_key = PublicKey::from_secret_key(&EC, &self.secret_key);
        SilentPaymentInput::spending(self.outpoint, self.script_pubkey.clone(), public_key).unwrap()
    }

    /// Build a transaction paying `recipient_address` and sign it, so that the public key of
    /// the input ends up in the witness where the receiver looks for it
    fn pay(&self, asset: AssetId, script_pubkey: Script, blinder: PublicKey) -> Transaction {
        let mut rng = elements::bitcoin::secp256k1::rand::thread_rng();
        let (txout, ..) = TxOut::new_last_confidential(
            &mut rng,
            &EC,
            SENT,
            asset,
            script_pubkey,
            blinder,
            &[self.secrets],
            &[],
        )
        .unwrap();

        let mut tx = Transaction {
            version: 2,
            lock_time: LockTime::ZERO,
            input: vec![TxIn {
                previous_output: self.outpoint,
                is_pegin: false,
                script_sig: Script::new(),
                sequence: Sequence::MAX,
                asset_issuance: Default::default(),
                witness: TxInWitness::empty(),
            }],
            output: vec![txout, TxOut::new_fee(FEE, asset)],
        };
        self.sign(&mut tx);
        tx
    }

    fn sign(&self, tx: &mut Transaction) {
        let public_key = BitcoinPublicKey::new(PublicKey::from_secret_key(&EC, &self.secret_key));
        let script_code =
            Address::p2pkh(&public_key, None, &AddressParams::ELEMENTS).script_pubkey();
        let sighash = SighashCache::new(&*tx).segwitv0_sighash(
            0,
            &script_code,
            Value::Explicit(FUNDED),
            EcdsaSighashType::All,
        );
        let message = Message::from_digest_slice(sighash.as_byte_array()).unwrap();
        let mut signature = EC
            .sign_ecdsa(&message, &self.secret_key)
            .serialize_der()
            .to_vec();
        signature.push(EcdsaSighashType::All as u8);
        tx.input[0].witness.script_witness = vec![signature, public_key.to_bytes()];
    }
}

fn setup() -> (
    Network,
    AssetId,
    Sender,
    SilentPaymentKeys,
    SilentPaymentWollet,
) {
    let network = Network::default_regtest();
    let asset = *network.policy_asset();
    let keys = SilentPaymentKeys::from_mnemonic(MNEMONIC, network, 0).unwrap();
    let wollet = SilentPaymentWollet::from_keys(network, &keys);
    (network, asset, Sender::new(asset), keys, wollet)
}

/// The sender derives the output from the address and its own inputs, the receiver finds it
/// scanning the transaction without any communication with the sender
#[test]
fn receive_silent_payment() {
    let (network, asset, sender, keys, mut wollet) = setup();

    let address = wollet.address();
    assert!(address.to_string().starts_with("elsp1q"));
    assert!(address.is_for_network(network));
    // the address is parsed back by the sender from its string representation
    let address = address.to_string().parse().unwrap();

    let inputs = vec![(sender.input(), Some(sender.secret_key))];
    let outputs = derive_outputs(&inputs, &[address]).unwrap();
    assert_eq!(outputs.len(), 1);
    let tx = sender.pay(
        asset,
        outputs[0].script_pubkey().clone(),
        outputs[0].blinding_public_key(),
    );

    let found = wollet
        .scan_transaction(&tx, &[sender.script_pubkey.clone()])
        .unwrap();
    assert_eq!(found.len(), 1);
    let output = &found[0];
    assert_eq!(output.outpoint(), OutPoint::new(tx.txid(), 0));
    assert_eq!(output.script_pubkey(), outputs[0].script_pubkey());
    assert_eq!(output.label(), None);

    // the receiver knows what it received even if the transaction is confidential
    assert!(matches!(output.txout().value, Value::Confidential(_)));
    assert!(matches!(output.txout().asset, Asset::Confidential(_)));
    let unblinded = output.unblinded().unwrap();
    assert_eq!(unblinded.value, SENT);
    assert_eq!(unblinded.asset, asset);

    // and it can spend it
    let secret_key = output
        .spending_secret_key(&keys.spend_secret_key())
        .unwrap();
    let (x_only, _) = secret_key.x_only_public_key(&EC);
    assert_eq!(&output.script_pubkey().as_bytes()[2..], &x_only.serialize());

    // scanning the same transaction again does not add the output twice
    assert!(wollet
        .scan_transaction(&tx, &[sender.script_pubkey.clone()])
        .unwrap()
        .is_empty());
    assert_eq!(wollet.outputs().count(), 1);

    // the received output can be tracked by a watch only wallet
    let descriptor = wollet.wollet_descriptor().unwrap();
    let script = descriptor.script_pubkey(Chain::External, 0).unwrap();
    assert_eq!(&script, output.script_pubkey());
    let address = descriptor.address(0, network.address_params()).unwrap();
    assert_eq!(
        address.blinding_pubkey.unwrap(),
        PublicKey::from_secret_key(&EC, &output.blinding_key())
    );
    // the descriptor round trips, it is what a wallet persists
    let parsed: WolletDescriptor = descriptor.to_string().parse().unwrap();
    assert_eq!(parsed.to_string(), descriptor.to_string());

    // and it can be used as an input of a transaction
    let utxos = wollet.external_utxos();
    assert_eq!(utxos.len(), 1);
    assert_eq!(utxos[0].unblinded.value, SENT);
}

/// Another wallet does not see the payment, not even when it is the one scanning
#[test]
fn other_wallet_does_not_receive() {
    let (network, asset, sender, _, wollet) = setup();
    let other = SilentPaymentKeys::from_mnemonic(OTHER_MNEMONIC, network, 0).unwrap();
    let mut other = SilentPaymentWollet::from_keys(network, &other);

    let address = wollet.address().to_string().parse().unwrap();
    let inputs = vec![(sender.input(), Some(sender.secret_key))];
    let outputs = derive_outputs(&inputs, &[address]).unwrap();
    let tx = sender.pay(
        asset,
        outputs[0].script_pubkey().clone(),
        outputs[0].blinding_public_key(),
    );

    let found = other
        .scan_transaction(&tx, &[sender.script_pubkey.clone()])
        .unwrap();
    assert!(found.is_empty());
    assert!(other.is_empty());
}

/// Two payments to the same address are two unrelated outputs
#[test]
fn two_payments_are_unlinkable() {
    let (_, asset, sender, _, mut wollet) = setup();
    let address: lwk_wollet::SilentPaymentAddress = wollet.address().to_string().parse().unwrap();

    let first = {
        let inputs = vec![(sender.input(), Some(sender.secret_key))];
        derive_outputs(&inputs, &[address.clone()]).unwrap()
    };

    // the same sender pays again, from a different input
    let mut other_sender = Sender::new(asset);
    other_sender.outpoint = OutPoint::new(Txid::from_slice(&[9u8; 32]).unwrap(), 1);
    let second = {
        let inputs = vec![(other_sender.input(), Some(other_sender.secret_key))];
        derive_outputs(&inputs, &[address]).unwrap()
    };

    assert_ne!(first[0].script_pubkey(), second[0].script_pubkey());
    assert_ne!(
        first[0].blinding_public_key(),
        second[0].blinding_public_key()
    );

    for (sender, outputs) in [(&sender, &first), (&other_sender, &second)] {
        let tx = sender.pay(
            asset,
            outputs[0].script_pubkey().clone(),
            outputs[0].blinding_public_key(),
        );
        let found = wollet
            .scan_transaction(&tx, &[sender.script_pubkey.clone()])
            .unwrap();
        assert_eq!(found.len(), 1);
    }
    assert_eq!(wollet.outputs().count(), 2);
}

/// Labels tell apart payments made to different addresses of the same wallet
#[test]
fn receive_on_labelled_address() {
    let (network, asset, sender, keys, mut wollet) = setup();
    wollet.add_label(7).unwrap();

    let address = wollet.labelled_address(7).unwrap();
    assert_ne!(
        address.spend_public_key(),
        wollet.address().spend_public_key()
    );
    assert_eq!(
        address.scan_public_key(),
        wollet.address().scan_public_key()
    );

    let inputs = vec![(sender.input(), Some(sender.secret_key))];
    let outputs = derive_outputs(&inputs, &[address]).unwrap();
    let tx = sender.pay(
        asset,
        outputs[0].script_pubkey().clone(),
        outputs[0].blinding_public_key(),
    );

    let found = wollet
        .scan_transaction(&tx, &[sender.script_pubkey.clone()])
        .unwrap();
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].label(), Some(7));
    assert_eq!(found[0].unblinded().unwrap().value, SENT);

    // a wallet that did not add the label does not see the payment
    let mut without_label =
        SilentPaymentWollet::new(network, keys.scan_secret_key(), keys.spend_public_key());
    assert!(without_label
        .scan_transaction(&tx, &[sender.script_pubkey.clone()])
        .unwrap()
        .is_empty());
}

/// A light client gets the tweak data from an index server instead of the spent outputs
#[test]
fn scan_with_tweak_data_from_index_server() {
    let (_, asset, sender, _, mut wollet) = setup();
    let address = wollet.address().to_string().parse().unwrap();
    let inputs = vec![(sender.input(), Some(sender.secret_key))];
    let outputs = derive_outputs(&inputs, &[address]).unwrap();
    let tx = sender.pay(
        asset,
        outputs[0].script_pubkey().clone(),
        outputs[0].blinding_public_key(),
    );

    // what the server computes and serves for this transaction
    let prevouts = [sender.script_pubkey.clone()];
    let inputs = transaction_inputs(&tx, &prevouts).unwrap();
    let tweak_data = lwk_wollet::silent_payments::tweak_data(&inputs)
        .unwrap()
        .unwrap();

    // the client matches the scripts it can derive against the block filter, then scans the
    // transactions that matched
    let candidates = wollet
        .scanner()
        .candidate_script_pubkeys(&tweak_data)
        .unwrap();
    assert!(tx
        .output
        .iter()
        .any(|txout| candidates.contains(&txout.script_pubkey)));

    let found = wollet
        .scan_transaction_with_tweak_data(&tx, &tweak_data)
        .unwrap();
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].unblinded().unwrap().value, SENT);
}

/// A transaction with no eligible input cannot pay to a silent payment address
#[test]
fn no_eligible_inputs() {
    let (_, asset, sender, _, mut wollet) = setup();
    let address = wollet.address().to_string().parse().unwrap();

    // a peg-in input spends coins on the bitcoin chain, it does not contribute its key
    let pegin = SilentPaymentInput::other(sender.outpoint, Script::new());
    assert!(derive_outputs(&[(pegin, None)], &[address]).is_err());

    let inputs = vec![(sender.input(), Some(sender.secret_key))];
    let outputs = derive_outputs(&inputs, &[wollet.address()]).unwrap();
    let mut tx = sender.pay(
        asset,
        outputs[0].script_pubkey().clone(),
        outputs[0].blinding_public_key(),
    );
    tx.input[0].is_pegin = true;
    assert!(wollet
        .scan_transaction(&tx, &[sender.script_pubkey.clone()])
        .unwrap()
        .is_empty());

    // the scripts of the spent outputs must match the inputs
    tx.input[0].is_pegin = false;
    assert!(wollet.scan_transaction(&tx, &[]).is_err());
}

/// The transaction is a valid Liquid transaction, not just a structure the scanner accepts
#[test]
fn transaction_is_well_formed() {
    let (_, asset, sender, _, wollet) = setup();
    let address = wollet.address().to_string().parse().unwrap();
    let inputs = vec![(sender.input(), Some(sender.secret_key))];
    let outputs = derive_outputs(&inputs, &[address]).unwrap();
    let tx = sender.pay(
        asset,
        outputs[0].script_pubkey().clone(),
        outputs[0].blinding_public_key(),
    );

    let bytes = serialize(&tx);
    let decoded: Transaction = elements::encode::deserialize(&bytes).unwrap();
    assert_eq!(decoded.txid(), tx.txid());
    assert!(tx.output[0].script_pubkey.is_v1_p2tr());
    assert!(tx.output[1].is_fee());
    assert!(tx.output[0].witness.rangeproof.is_some());
    assert!(tx.output[0].witness.surjection_proof.is_some());

    // the output pays to the confidential address the sender could have shown to the user
    let address = outputs[0].address(&AddressParams::ELEMENTS);
    assert!(address.is_blinded());
    assert_eq!(address.script_pubkey(), tx.output[0].script_pubkey);
    assert_eq!(Address::from_str(&address.to_string()).unwrap(), address);
}

/// The test vectors published in the ELIP, any change here is a change to the specification
#[test]
fn elip_test_vectors() {
    let cases = [
        (
            Network::Liquid,
            "e4284c50d48a373098d44638e3c9d6f6eca770aaa54562eb8cdc0cff8cd3b550",
            "ca29645fc7167f2810c909fa52bcd177b82aa3bfbe999ef579a6607926dc7f93",
            "lqsp1qqgvh6dnt5eyvw54a0dvt4er7tkq0hdefm39sta8adph4vufq650tcqc9sw44xujvc7ejg4w5lt4zxpvc3fk446qdcrrmsgg9cnax8ujj7spnvlwq",
            "lqsp1qqgvh6dnt5eyvw54a0dvt4er7tkq0hdefm39sta8adph4vufq650tcqmujrcapnql6np3dpdjwswdwdcs0h3rs637agjsaeac6thmlcfpyy2eg0eh",
            "lqsp1qqgvh6dnt5eyvw54a0dvt4er7tkq0hdefm39sta8adph4vufq650tcq3789yp3lv5nvyyvhucjttyanypmzdg7qpadykuz7xd9zenvh6dwylcw0sj",
        ),
        (
            Network::TestnetLiquid,
            "38658693c017c46fd6b8bb94b8766c123cd5baf6026338305b6f59f82b36f9c0",
            "9fd37137e760930c7208fa905e991c78c522689d237a220b2820c3ddb4c745a8",
            "tlqsp1qqdpels3srq45dlezqvk20t3dlueftry6p5thc7msjm0s6jm3g84jzq5rxzzunfck6d45va2jcqxk429agt3e4klf3vzmcgp3zqthryhhqgu2k23l",
            "tlqsp1qqdpels3srq45dlezqvk20t3dlueftry6p5thc7msjm0s6jm3g84jzq4y0sngs4cjaw6cxuy7p6rlslx6h54lmppe9kx4r2ylf63tkem26qqt6u7t",
            "tlqsp1qqdpels3srq45dlezqvk20t3dlueftry6p5thc7msjm0s6jm3g84jzqknp3yhxtxhcvkczk4vfvdx0z8pfff3n78nelgppy4l2sss3famcgq974m5",
        ),
        (
            Network::default_regtest(),
            "38658693c017c46fd6b8bb94b8766c123cd5baf6026338305b6f59f82b36f9c0",
            "9fd37137e760930c7208fa905e991c78c522689d237a220b2820c3ddb4c745a8",
            "elsp1qqdpels3srq45dlezqvk20t3dlueftry6p5thc7msjm0s6jm3g84jzq5rxzzunfck6d45va2jcqxk429agt3e4klf3vzmcgp3zqthryhhqgd4lgkm",
            "elsp1qqdpels3srq45dlezqvk20t3dlueftry6p5thc7msjm0s6jm3g84jzq4y0sngs4cjaw6cxuy7p6rlslx6h54lmppe9kx4r2ylf63tkem26q35n7e0",
            "elsp1qqdpels3srq45dlezqvk20t3dlueftry6p5thc7msjm0s6jm3g84jzqknp3yhxtxhcvkczk4vfvdx0z8pfff3n78nelgppy4l2sss3famcg36hhus",
        ),
    ];

    for (network, scan, spend, address, change, label_1) in cases {
        let keys = SilentPaymentKeys::from_mnemonic(MNEMONIC, network, 0).unwrap();
        assert_eq!(keys.scan_secret_key().display_secret().to_string(), scan);
        assert_eq!(keys.spend_secret_key().display_secret().to_string(), spend);
        assert_eq!(keys.address(network).to_string(), address);
        assert_eq!(
            keys.labelled_address(network, 0).unwrap().to_string(),
            change
        );
        assert_eq!(
            keys.labelled_address(network, 1).unwrap().to_string(),
            label_1
        );
    }

    // a payment on Liquid, from a P2WPKH input to the address above
    let network = Network::Liquid;
    let keys = SilentPaymentKeys::from_mnemonic(MNEMONIC, network, 0).unwrap();
    let secret_key = SecretKey::from_slice(&[11u8; 32]).unwrap();
    let public_key = PublicKey::from_secret_key(&EC, &secret_key);
    let prevout = Address::p2wpkh(
        &BitcoinPublicKey::new(public_key),
        None,
        &AddressParams::LIQUID,
    );
    let outpoint = OutPoint::new(
        Txid::from_str("f4184fc596403b9d638783cf57adfe4c75c605f6356fbc91338530e9831e9e16").unwrap(),
        0,
    );
    assert_eq!(
        public_key.to_string(),
        "02552c630b64b54bf50210c9e253d38bd4949c72e22873500f6285c2bede312a84"
    );
    assert_eq!(
        format!("{:x}", prevout.script_pubkey()),
        "0014db3f00d429f2715383cc594258ec11d6de526697"
    );

    let input =
        SilentPaymentInput::spending(outpoint, prevout.script_pubkey(), public_key).unwrap();
    let tweak_data = lwk_wollet::silent_payments::tweak_data(&[input.clone()])
        .unwrap()
        .unwrap();
    assert_eq!(
        tweak_data.to_string(),
        "03398173f560782d934ddf4f5a291c47fd0866d6e26a97a7407b810e1873e34777"
    );

    let outputs = derive_outputs(&[(input, Some(secret_key))], &[keys.address(network)]).unwrap();
    assert_eq!(
        format!("{:x}", outputs[0].script_pubkey()),
        "512092a9d712661ac4c1ebd5e3953a5af8a60037fb69e7c559ecacbce41a050262d7"
    );
    assert_eq!(
        outputs[0].blinding_public_key().to_string(),
        "0368c38c6542751c6c8778c31f60043d3dd5efa3dd03ccc1dff7b7c4ab6ae3afb8"
    );
    assert_eq!(
        outputs[0].address(&AddressParams::LIQUID).to_string(),
        "lq1pqd5v8rr9gf63cmy80rp37cqy857atmarm5pueswl77muf2m2uwhm3y4f6ufxvxkyc84atcu48fd03fsqxlakne79t8k2e08yrgzsyckhf2sstv683ytv"
    );

    // and what the receiver derives when it finds the output
    let sender = Sender {
        secret_key,
        outpoint,
        script_pubkey: prevout.script_pubkey(),
        secrets: TxOutSecrets::new(
            *network.policy_asset(),
            AssetBlindingFactor::zero(),
            FUNDED,
            ValueBlindingFactor::zero(),
        ),
    };
    let tx = sender.pay(
        *network.policy_asset(),
        outputs[0].script_pubkey().clone(),
        outputs[0].blinding_public_key(),
    );
    let mut wollet = SilentPaymentWollet::from_keys(network, &keys);
    let found = wollet
        .scan_transaction(&tx, &[prevout.script_pubkey()])
        .unwrap();
    assert_eq!(found.len(), 1);
    assert_eq!(
        found[0].tweak().display_secret().to_string(),
        "a4c41218034595e818fce768dded0e52409abfe23501fb94e6787c32debad2bd"
    );
    assert_eq!(
        found[0].blinding_key().display_secret().to_string(),
        "f01c7948615e9ead2e612cf325857a605346bdd240896263ec888d2d4b7d1e27"
    );
    assert_eq!(
        found[0]
            .spending_secret_key(&keys.spend_secret_key())
            .unwrap()
            .display_secret()
            .to_string(),
        "6eed7677ca5c151029c5f16330a9dfcb3e1686bb4452fa4ea04c7e1f3561110f"
    );
    assert_eq!(found[0].unblinded().unwrap().value, SENT);
}
