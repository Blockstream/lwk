use std::collections::BTreeMap;

use elements::{OutPoint, Script, Transaction};
use lwk_common::Network;

use crate::secp256k1::{PublicKey, SecretKey};
use crate::{cache::Height, Error, ExternalUtxo, WolletDescriptor};

use super::{
    SilentPaymentAddress, SilentPaymentError, SilentPaymentKeys, SilentPaymentScanner,
    SilentPaymentTxOut,
};

/// A watch only wallet detecting the payments made to a silent payment address.
///
/// Silent payment outputs cannot be derived in advance, they are discovered scanning the
/// blockchain. Once discovered they are ordinary confidential taproot outputs, so they can be
/// tracked by a [`crate::Wollet`] built on [`SilentPaymentWollet::wollet_descriptor`], which
/// takes care of their balance, of the transaction history and of the outputs being spent.
#[derive(Debug, Clone)]
pub struct SilentPaymentWollet {
    network: Network,
    scanner: SilentPaymentScanner,
    outputs: BTreeMap<OutPoint, SilentPaymentTxOut>,
    last_scanned_height: Option<Height>,
}

impl SilentPaymentWollet {
    /// Create a wallet detecting the payments to the address made of the given keys
    pub fn new(network: Network, scan_secret_key: SecretKey, spend_public_key: PublicKey) -> Self {
        Self {
            network,
            scanner: SilentPaymentScanner::new(scan_secret_key, spend_public_key),
            outputs: BTreeMap::new(),
            last_scanned_height: None,
        }
    }

    /// Create a wallet from the keys of a signer, note that the spend secret key is not kept
    pub fn from_keys(network: Network, keys: &SilentPaymentKeys) -> Self {
        Self::new(network, keys.scan_secret_key(), keys.spend_public_key())
    }

    /// Also detect the payments made to the address labelled with `m`
    pub fn add_label(&mut self, m: u32) -> Result<(), SilentPaymentError> {
        self.scanner.add_label(m)
    }

    /// The network of this wallet
    pub fn network(&self) -> Network {
        self.network
    }

    /// The address to give out to receive payments
    pub fn address(&self) -> SilentPaymentAddress {
        self.scanner.address(self.network.into())
    }

    /// The address labelled with `m`, see [`SilentPaymentScanner::labelled_address`]
    pub fn labelled_address(&self, m: u32) -> Result<SilentPaymentAddress, SilentPaymentError> {
        self.scanner.labelled_address(self.network.into(), m)
    }

    /// The scanner of this wallet, to detect payments without adding them to the wallet
    pub fn scanner(&self) -> &SilentPaymentScanner {
        &self.scanner
    }

    /// Scan a transaction and add the outputs paying to this wallet.
    ///
    /// `prevout_script_pubkeys` are the scripts of the outputs spent by the transaction, in the
    /// same order as its inputs.
    ///
    /// Returns the outputs discovered by this call, outputs already known are not returned.
    pub fn scan_transaction(
        &mut self,
        tx: &Transaction,
        prevout_script_pubkeys: &[Script],
    ) -> Result<Vec<SilentPaymentTxOut>, SilentPaymentError> {
        let found = self.scanner.scan_transaction(tx, prevout_script_pubkeys)?;
        Ok(self.insert(found))
    }

    /// Scan a transaction with the tweak data obtained from an index server, see
    /// [`SilentPaymentScanner::scan_transaction_with_tweak_data`]
    pub fn scan_transaction_with_tweak_data(
        &mut self,
        tx: &Transaction,
        tweak_data: &PublicKey,
    ) -> Result<Vec<SilentPaymentTxOut>, SilentPaymentError> {
        let found = self
            .scanner
            .scan_transaction_with_tweak_data(tx, tweak_data)?;
        Ok(self.insert(found))
    }

    fn insert(&mut self, found: Vec<SilentPaymentTxOut>) -> Vec<SilentPaymentTxOut> {
        found
            .into_iter()
            .filter(|txout| {
                self.outputs
                    .insert(txout.outpoint(), txout.clone())
                    .is_none()
            })
            .collect()
    }

    /// The outputs discovered so far, ordered by outpoint.
    ///
    /// Note that an output could have been spent already, ask a [`crate::Wollet`] built on
    /// [`SilentPaymentWollet::wollet_descriptor`] for the unspent ones.
    pub fn outputs(&self) -> impl Iterator<Item = &SilentPaymentTxOut> {
        self.outputs.values()
    }

    /// Whether no output has been discovered yet
    pub fn is_empty(&self) -> bool {
        self.outputs.is_empty()
    }

    /// The output with the given outpoint, if it belongs to this wallet
    pub fn output(&self, outpoint: &OutPoint) -> Option<&SilentPaymentTxOut> {
        self.outputs.get(outpoint)
    }

    /// The height of the last block scanned, to resume scanning after a restart
    pub fn last_scanned_height(&self) -> Option<Height> {
        self.last_scanned_height
    }

    /// See [`SilentPaymentWollet::last_scanned_height`]
    pub fn set_last_scanned_height(&mut self, height: Height) {
        self.last_scanned_height = Some(height);
    }

    /// The descriptor tracking the outputs discovered so far.
    ///
    /// It changes every time an output is discovered, a [`crate::Wollet`] built on it must be
    /// rebuilt to see the new outputs.
    pub fn wollet_descriptor(&self) -> Result<WolletDescriptor, Error> {
        if self.outputs.is_empty() {
            return Err(Error::Generic(
                "no silent payment output has been discovered yet".into(),
            ));
        }
        let descriptor: Vec<String> = self
            .outputs
            .values()
            .map(|txout| {
                format!(
                    "{}:{:x}",
                    txout.blinding_key().display_secret(),
                    txout.script_pubkey()
                )
            })
            .collect();
        descriptor.join(",").parse()
    }

    /// The outputs discovered so far, as inputs for
    /// [`crate::TxBuilder::add_external_utxos`].
    ///
    /// Signing them requires the secret key returned by
    /// [`SilentPaymentTxOut::spending_secret_key`], a taproot key path signature over the
    /// output key without the BIP341 tweak.
    pub fn external_utxos(&self) -> Vec<ExternalUtxo> {
        self.outputs
            .values()
            .filter_map(|txout| txout.to_external_utxo())
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::silent_payments::test_vectors::{taproot_script, test_vectors, transaction, Vin};
    use crate::silent_payments::tweak_data;

    /// A wallet built on the descriptor of the discovered outputs sees the same scripts
    #[test]
    fn wollet_descriptor() {
        let network = Network::default_regtest();
        let mut checked = 0;
        for vector in test_vectors() {
            for receiving in &vector.receiving {
                let scanner = receiving.scanner();
                let mut wollet = SilentPaymentWollet {
                    network,
                    scanner: scanner.clone(),
                    outputs: BTreeMap::new(),
                    last_scanned_height: None,
                };
                assert!(wollet.is_empty());
                assert!(wollet.wollet_descriptor().is_err());

                let inputs: Vec<_> = receiving.given.vin.iter().map(Vin::input).collect();
                let scripts: Vec<_> = receiving
                    .given
                    .outputs
                    .iter()
                    .map(|o| taproot_script(o))
                    .collect();
                let tx = transaction(&inputs, &scripts);
                let Some(tweak_data) = tweak_data(&inputs).unwrap() else {
                    continue;
                };
                let found = wollet
                    .scan_transaction_with_tweak_data(&tx, &tweak_data)
                    .unwrap();
                if found.is_empty() {
                    continue;
                }
                // scanning twice does not duplicate the outputs
                assert!(wollet
                    .scan_transaction_with_tweak_data(&tx, &tweak_data)
                    .unwrap()
                    .is_empty());
                assert_eq!(wollet.outputs().count(), found.len());

                let descriptor = wollet.wollet_descriptor().unwrap();
                for (i, output) in wollet.outputs().enumerate() {
                    let script = descriptor
                        .script_pubkey(crate::Chain::External, i as u32)
                        .unwrap();
                    assert_eq!(&script, output.script_pubkey());
                    let address = descriptor
                        .address(i as u32, network.address_params())
                        .unwrap();
                    assert!(address.is_blinded());
                }
                checked += 1;
            }
        }
        assert!(checked > 0);
    }
}
