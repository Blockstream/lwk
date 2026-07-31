//! Shared construction of silent-payment test transactions.

use crate::elements::confidential::{Asset, Value};
use crate::elements::{
    AssetId, LockTime, OutPoint, Script, Transaction, TxIn, TxOut, TxOutWitness,
};
use crate::secp256k1::SecretKey;
use crate::silentpayments::{
    SilentPaymentAddress, SilentPaymentScan, SilentPaymentScanMaterial,
    SilentPaymentSender, SpTxOutBuilder,
};
use lwk_test_util::ElementsTestData;

pub(crate) struct SilentPaymentTestData;

impl SilentPaymentTestData {
    pub(crate) fn secret_key(byte: u8) -> SecretKey {
        ElementsTestData::secret_key(byte)
    }

    pub(crate) fn outpoint(txid_byte: u8, vout: u32) -> OutPoint {
        ElementsTestData::outpoint(txid_byte, vout)
    }

    pub(crate) fn txid(byte: u8) -> crate::elements::Txid {
        ElementsTestData::txid(byte)
    }

    pub(crate) fn asset() -> AssetId {
        AssetId::from_slice(&[0x42u8; 32]).unwrap()
    }

    pub(crate) fn material(scan: u8, spend: u8) -> SilentPaymentScanMaterial {
        SilentPaymentScanMaterial::new(
            crate::silentpayments::SilentPaymentAccount::liquid_testnet(0),
            ElementsTestData::secret_key(scan),
            ElementsTestData::public_key(spend),
        )
    }
}

/// A silent payment built for a test.
pub(crate) struct SpPayment {
    pub(crate) tx: Transaction,
    /// The scriptPubKeys the transaction's inputs spend.
    pub(crate) prevouts: Vec<(OutPoint, Script)>,
}

impl SpPayment {
    /// Resolves prevout scripts, borrowing from the fixture rather than the argument.
    pub(crate) fn prevout_lookup<'a>(&'a self) -> impl FnMut(&OutPoint) -> Option<&'a Script> {
        move |o: &OutPoint| self.prevouts.iter().find(|(p, _)| p == o).map(|(_, s)| s)
    }
}

/// Builds silent-payment transactions for tests, defaulting to a two-input payment
/// at index 0.
pub(crate) struct SpPaymentBuilder {
    inputs: Vec<(OutPoint, SecretKey)>,
    k: u32,
    value: u64,
    asset: AssetId,
    /// Whether to append a second, non-silent output.
    extra_output: bool,
}

impl Default for SpPaymentBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl SpPaymentBuilder {
    pub(crate) fn new() -> Self {
        SpPaymentBuilder {
            inputs: vec![
                (
                    ElementsTestData::outpoint(0x22, 1),
                    ElementsTestData::secret_key(0xA1),
                ),
                (
                    ElementsTestData::outpoint(0x11, 0),
                    ElementsTestData::secret_key(0xA2),
                ),
            ],
            k: 0,
            value: 50_000,
            asset: SilentPaymentTestData::asset(),
            extra_output: true,
        }
    }

    pub(crate) fn with_inputs(mut self, inputs: &[(OutPoint, SecretKey)]) -> Self {
        self.inputs = inputs.to_vec();
        self
    }

    pub(crate) fn with_value(mut self, value: u64) -> Self {
        self.value = value;
        self
    }

    /// Build a payment to `address`.
    pub(crate) fn build(self, address: &SilentPaymentAddress) -> SpPayment {
        let sender = SilentPaymentSender::from_inputs(&self.inputs)
            .expect("the fixture's inputs must aggregate");
        let output = sender.derive_output(address, self.k);

        let (sp_txout, _) =
            SpTxOutBuilder::build(&output, self.asset, self.value, &mut rand::thread_rng())
                .expect("building the silent payment output must succeed");

        let mut outputs = vec![sp_txout];
        if self.extra_output {
            outputs.push(TxOut {
                asset: Asset::Explicit(self.asset),
                value: Value::Explicit(500),
                nonce: Default::default(),
                script_pubkey: Script::new(),
                witness: TxOutWitness::default(),
            });
        }

        let tx = Transaction {
            version: 2,
            lock_time: LockTime::ZERO,
            input: self.inputs.iter().map(Self::witness_input).collect(),
            output: outputs,
        };

        SpPayment {
            prevouts: self.inputs.iter().map(Self::prevout).collect(),
            tx,
        }
    }

    /// Build a payment to the address `material` publishes.
    pub(crate) fn build_for(self, material: &SilentPaymentScanMaterial) -> SpPayment {
        self.build(&material.address())
    }

    /// A P2WPKH input whose witness carries the pubkey the key is recovered from.
    fn witness_input((outpoint, key): &(OutPoint, SecretKey)) -> TxIn {
        ElementsTestData::p2wpkh_input(*outpoint, key)
    }

    fn prevout((outpoint, key): &(OutPoint, SecretKey)) -> (OutPoint, Script) {
        (*outpoint, ElementsTestData::p2wpkh(key))
    }
}
