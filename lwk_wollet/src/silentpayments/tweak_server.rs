//! The tweak/index server model (design §4.5, BIP0352-index-server-specification).

use crate::secp256k1::{PublicKey, Scalar};
use crate::silentpayments::{ObservedInputs, SilentPaymentInputError};
use crate::util::EC;

/// A server-published partial tweak: `T = input_hash · A`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PartialTweak(PublicKey);

impl PartialTweak {
    /// `T = input_hash · A`.
    pub fn new(a_pubkey: &PublicKey, input_hash: &Scalar) -> Self {
        PartialTweak(
            a_pubkey
                .mul_tweak(&EC, input_hash)
                .expect("partial tweak point mul"),
        )
    }

    /// Compute a partial tweak directly from an observer's aggregated inputs.
    pub fn from_observed(observed: &ObservedInputs) -> Self {
        Self::new(&observed.a_pubkey, &observed.input_hash)
    }

    /// Computes a partial tweak from observed input keys.
    pub fn from_inputs(
        inputs: &[(crate::elements::OutPoint, PublicKey)],
    ) -> Result<Self, SilentPaymentInputError> {
        Ok(Self::from_observed(&ObservedInputs::aggregate(inputs)?))
    }

    /// The underlying point, as published by the server and consumed by clients.
    pub fn as_pubkey(&self) -> &PublicKey {
        &self.0
    }
}

impl From<PartialTweak> for PublicKey {
    fn from(t: PartialTweak) -> Self {
        t.0
    }
}

/// A source of per-transaction BIP-352 partial tweaks.
pub trait SilentPaymentTweakClient {
    /// The error type returned by the backend.
    type Error;

    /// Return the partial tweaks `T = input_hash·A` for every SP-eligible
    /// transaction in the block at `height`.
    fn tweaks(&self, height: u32) -> Result<Vec<PartialTweak>, Self::Error>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::silentpayments::test_fixture::SilentPaymentTestData as Data;
    use crate::silentpayments::{SharedSecret, SilentPaymentInputs};

    #[test]
    fn every_route_to_a_partial_tweak_agrees() {
        let keys = Data::material(0x11, 0x22);
        let inputs = [
            (Data::outpoint(0x10, 0), Data::secret_key(0x31)),
            (Data::outpoint(0x20, 1), Data::secret_key(0x32)),
        ];
        let agg = SilentPaymentInputs::aggregate(&inputs).unwrap();
        let observed: Vec<_> = inputs
            .iter()
            .map(|(o, s)| (*o, s.public_key(&EC)))
            .collect();

        let t = PartialTweak::new(&agg.a_pubkey, &agg.input_hash);
        assert_eq!(t, PartialTweak::from_observed(&agg.observed()));
        assert_eq!(t, PartialTweak::from_inputs(&observed).unwrap());

        let client = SharedSecret::from_partial_tweak(&keys.scan_seckey(), t.as_pubkey());
        let sender = SharedSecret::for_sender(&keys.scan_seckey().public_key(&EC), &agg);
        assert_eq!(client, sender);
    }
}
