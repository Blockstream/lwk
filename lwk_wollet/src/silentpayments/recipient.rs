//! Pending silent-payment recipients and input-dependent resolution.

use crate::elements::{AssetId, OutPoint};
use crate::silentpayments::{InputKey, SilentPaymentAddress, SilentPaymentSender};
use crate::{Error, Recipient};
use std::collections::HashMap;

/// A pending silent payment: who to pay, how much, and of which asset.
///
/// Created by [`crate::TxBuilder::add_silent_payment_recipient()`].
#[derive(Debug, Clone)]
pub struct SilentPaymentRecipient {
    /// The receiver's reusable silent payment address.
    pub address: SilentPaymentAddress,

    /// The amount to send, in satoshi.
    pub satoshi: u64,

    /// The asset to send.
    pub asset: AssetId,
}

impl SilentPaymentRecipient {
    /// Queue a payment of `satoshi` units of `asset` to `address`.
    pub fn new(address: SilentPaymentAddress, satoshi: u64, asset: AssetId) -> Self {
        Self {
            address,
            satoshi,
            asset,
        }
    }

    /// Resolves recipients using the transaction's eligible input keys.
    pub fn resolve_all(
        recipients: &[SilentPaymentRecipient],
        inputs: &[(OutPoint, InputKey)],
        extra_outpoints: &[OutPoint],
    ) -> Result<Vec<ResolvedSilentPayment>, Error> {
        if recipients.is_empty() {
            return Ok(vec![]);
        }

        let sender = SilentPaymentSender::from_input_keys(inputs, extra_outpoints)?;

        let mut next_index_by_scan: HashMap<_, u32> = HashMap::new();
        recipients
            .iter()
            .map(|r| {
                let k = next_index_by_scan.entry(r.address.scan).or_insert(0);
                let index = *k;
                *k = (*k)
                    .checked_add(1)
                    .ok_or(Error::SilentPaymentRequiresKeys(0))?;
                let k = index;
                let out = sender
                    .try_derive_output(&r.address, k)
                    .ok_or(crate::silentpayments::SilentPaymentInputError::TooManyOutputs)?;

                Ok(ResolvedSilentPayment {
                    recipient: Recipient {
                        satoshi: r.satoshi,
                        script_pubkey: out.script_pubkey(),
                        blinding_pubkey: Some(out.blinding_pubkey),
                        asset: r.asset,
                    },
                    output: out,
                })
            })
            .collect()
    }
}

/// A resolved recipient and its derived silent-payment output.
#[derive(Debug, Clone)]
pub struct ResolvedSilentPayment {
    /// The recipient as passed to the transaction builder.
    pub recipient: Recipient,

    /// The derived output the recipient resolves to.
    pub output: crate::silentpayments::SilentPaymentOutput,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::silentpayments::test_fixture::SilentPaymentTestData as Data;
    use crate::silentpayments::SilentPaymentScan;

    /// Several silent payments in one transaction must land on DISTINCT outputs:
    /// each recipient takes the next index `k`, per BIP-352.
    #[test]
    fn multiple_recipients_get_distinct_indices() {
        let a = Data::material(0x11, 0x22);
        let b = Data::material(0x33, 0x44);
        let asset = AssetId::from_slice(&[0x42u8; 32]).unwrap();
        let inputs = [(
            Data::outpoint(0x10, 0),
            InputKey::Plain(Data::secret_key(0x31)),
        )];

        // Two payments to DIFFERENT addresses.
        let pending = [
            SilentPaymentRecipient::new(a.address(), 1_000, asset),
            SilentPaymentRecipient::new(b.address(), 2_000, asset),
        ];
        let resolved = SilentPaymentRecipient::resolve_all(&pending, &inputs, &[]).unwrap();
        assert_ne!(
            resolved[0].recipient.script_pubkey,
            resolved[1].recipient.script_pubkey
        );

        // Two payments to the SAME address must also differ — this is the case that
        // would silently collapse into one output if `k` were not incremented,
        // burning the second payment.
        let same = [
            SilentPaymentRecipient::new(a.address(), 1_000, asset),
            SilentPaymentRecipient::new(a.address(), 2_000, asset),
        ];
        let resolved = SilentPaymentRecipient::resolve_all(&same, &inputs, &[]).unwrap();
        assert_ne!(
            resolved[0].recipient.script_pubkey, resolved[1].recipient.script_pubkey,
            "two payments to one address must not collapse onto the same output"
        );
        assert_eq!(resolved[0].recipient.satoshi, 1_000);
        assert_eq!(resolved[1].recipient.satoshi, 2_000);
    }

    /// No recipients → no inputs needed, and no error. Resolution must not demand
    /// input keys from a transaction that has no silent payments in it.
    #[test]
    fn resolving_nothing_is_a_no_op() {
        assert!(SilentPaymentRecipient::resolve_all(&[], &[], &[])
            .unwrap()
            .is_empty());
    }
}
