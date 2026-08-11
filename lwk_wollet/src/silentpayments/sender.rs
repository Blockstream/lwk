//! The sending side of a silent payment.

use crate::secp256k1::SecretKey;
use crate::silentpayments::inputs::InputHasher;
use crate::silentpayments::{
    SharedSecret, SilentPaymentAddress, SilentPaymentInputs, SilentPaymentOutput,
    SilentPaymentScanner,
};
use crate::util::EC;

/// Derives outputs for a silent-payment address.
#[derive(Debug, Clone, Copy)]
pub struct SilentPaymentSender {
    inputs: SilentPaymentInputs,
}

impl SilentPaymentSender {
    /// Build a sender from the wallet's aggregated eligible inputs.
    pub fn new(inputs: SilentPaymentInputs) -> Self {
        SilentPaymentSender { inputs }
    }

    /// Aggregate `(outpoint, private_key)` pairs and build the sender in one step.
    pub fn from_inputs(
        inputs: &[(crate::elements::OutPoint, SecretKey)],
    ) -> Result<Self, crate::silentpayments::SilentPaymentInputError> {
        Ok(Self::new(SilentPaymentInputs::aggregate(inputs)?))
    }

    /// Aggregates tagged inputs and keyless-input outpoints.
    pub fn from_input_keys(
        inputs: &[(crate::elements::OutPoint, crate::silentpayments::InputKey)],
        extra_outpoints: &[crate::elements::OutPoint],
    ) -> Result<Self, crate::silentpayments::SilentPaymentInputError> {
        Ok(Self::new(
            SilentPaymentInputs::aggregate_with_extra_outpoints(inputs, extra_outpoints)?,
        ))
    }

    /// The aggregated inputs backing this sender.
    pub fn inputs(&self) -> &SilentPaymentInputs {
        &self.inputs
    }

    /// Derives `S = input_hash · a · B_scan`.
    pub fn shared_secret(&self, address: &SilentPaymentAddress) -> SharedSecret {
        SharedSecret::for_sender(&address.scan, &self.inputs)
    }

    /// Derives output `k`; panics when `k >= K_MAX`.
    pub fn derive_output(&self, address: &SilentPaymentAddress, k: u32) -> SilentPaymentOutput {
        self.try_derive_output(address, k)
            .expect("output index within K_max")
    }

    /// Derives output `k`, or `None` when `k >= K_MAX`.
    pub fn try_derive_output(
        &self,
        address: &SilentPaymentAddress,
        k: u32,
    ) -> Option<SilentPaymentOutput> {
        if k >= SilentPaymentScanner::K_MAX {
            return None;
        }
        Some(self.shared_secret(address).derive_output(&address.spend, k))
    }

    /// Derives an output from a summed key and serialized outpoint.
    pub fn derive_output_from_raw(
        address: &SilentPaymentAddress,
        a_sum: &SecretKey,
        outpoint_l: &[u8],
        k: u32,
    ) -> SilentPaymentOutput {
        let a_pubkey = a_sum.public_key(&EC);
        let inputs = SilentPaymentInputs {
            a_sum: *a_sum,
            a_pubkey,
            input_hash: InputHasher::hash(outpoint_l, &a_pubkey),
        };
        Self::new(inputs).derive_output(address, k)
    }
}
