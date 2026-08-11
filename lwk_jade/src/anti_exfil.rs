//! Host-side implementation of the ECDSA anti-exfil protocol.
//!
//! The construction matches libsecp256k1-zkp's `ecdsa_s2c` module:
//! <https://github.com/BlockstreamResearch/secp256k1-zkp/blob/master/include/secp256k1_ecdsa_s2c.h>

// TODO: Replace this implementation with rust-secp256k1-zkp's ECDSA anti-exfil bindings once
// available: https://github.com/BlockstreamResearch/rust-secp256k1-zkp/issues/100

use elements::{
    hashes::Hash,
    secp256k1_zkp::{ecdsa::Signature, Message, PublicKey, Scalar, Secp256k1, Verification},
};
use rand::RngCore;

elements::hashes::sha256t_hash_newtype! {
    struct S2cDataTag = hash_str("s2c/ecdsa/data");
    struct S2cDataHash(_);

    struct S2cPointTag = hash_str("s2c/ecdsa/point");
    struct S2cPointHash(_);
}

const CURVE_ORDER: [u8; 32] = [
    0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xfe,
    0xba, 0xae, 0xdc, 0xe6, 0xaf, 0x48, 0xa0, 0x3b, 0xbf, 0xd2, 0x5e, 0x8c, 0xd0, 0x36, 0x41, 0x41,
];

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum VerifyError {
    InvalidSignerCommitment,
    InvalidSignature,
    VerificationFailed,
}

pub(crate) fn new_host_entropy() -> Result<[u8; 32], rand::Error> {
    let mut entropy = [0u8; 32];
    rand::rngs::OsRng.try_fill_bytes(&mut entropy)?;
    Ok(entropy)
}

pub(crate) fn host_commitment(entropy: &[u8; 32]) -> [u8; 32] {
    S2cDataHash::hash(entropy).to_byte_array()
}

pub(crate) fn verify<C: Verification>(
    secp: &Secp256k1<C>,
    public_key: &PublicKey,
    message: &Message,
    host_entropy: &[u8; 32],
    signer_commitment: &[u8],
    signature: &[u8],
    expected_sighash: u8,
) -> Result<(), VerifyError> {
    let opening = PublicKey::from_slice(signer_commitment)
        .map_err(|_| VerifyError::InvalidSignerCommitment)?;

    let (&sighash, der_signature) = signature
        .split_last()
        .ok_or(VerifyError::InvalidSignature)?;
    if sighash != expected_sighash {
        return Err(VerifyError::InvalidSignature);
    }

    let signature =
        Signature::from_der(der_signature).map_err(|_| VerifyError::InvalidSignature)?;
    secp.verify_ecdsa(message, &signature, public_key)
        .map_err(|_| VerifyError::VerificationFailed)?;

    let mut commitment_data = [0u8; 65];
    commitment_data[..33].copy_from_slice(&opening.serialize());
    commitment_data[33..].copy_from_slice(host_entropy);
    let tweak = S2cPointHash::hash(&commitment_data).to_byte_array();
    let tweak = Scalar::from_be_bytes(tweak).map_err(|_| VerifyError::VerificationFailed)?;
    let committed_nonce = opening
        .add_exp_tweak(secp, &tweak)
        .map_err(|_| VerifyError::VerificationFailed)?;

    let compact_signature = signature.serialize_compact();
    let mut nonce_x = [0u8; 32];
    nonce_x.copy_from_slice(&committed_nonce.serialize()[1..]);
    reduce_mod_curve_order(&mut nonce_x);

    // ECDSA encodes r as the committed nonce's x-coordinate modulo the curve order.
    if compact_signature[..32] != nonce_x {
        return Err(VerifyError::VerificationFailed);
    }

    Ok(())
}

fn reduce_mod_curve_order(value: &mut [u8; 32]) {
    if *value < CURVE_ORDER {
        return;
    }

    let mut borrow = false;
    for (value_byte, order_byte) in value.iter_mut().zip(CURVE_ORDER).rev() {
        let (result, first_borrow) = value_byte.overflowing_sub(order_byte);
        let (result, second_borrow) = result.overflowing_sub(u8::from(borrow));
        *value_byte = result;
        borrow = first_borrow || second_borrow;
    }
    debug_assert!(!borrow);
}

#[cfg(test)]
mod tests {
    use elements::secp256k1_zkp::{Message, PublicKey, Secp256k1, SecretKey};

    use super::{host_commitment, reduce_mod_curve_order, verify, VerifyError, CURVE_ORDER};

    #[test]
    fn host_commitment_matches_libwally() {
        let entropy =
            hex::decode("3f5540b9336af9bdd50a5b7f69fc2045a12e3b3e0740f7461902d882bf8a8820")
                .unwrap();
        let entropy: [u8; 32] = entropy.try_into().unwrap();
        assert_eq!(
            hex::encode(host_commitment(&entropy)),
            "7b61fad27ce2d95abca09f76bd7226e50212a8542f3ca274ee546cec4bc5c3bb"
        );
    }

    #[test]
    fn curve_order_reduction() {
        let mut below = CURVE_ORDER;
        below[31] -= 1;
        let expected = below;
        reduce_mod_curve_order(&mut below);
        assert_eq!(below, expected);

        let mut order = CURVE_ORDER;
        reduce_mod_curve_order(&mut order);
        assert_eq!(order, [0u8; 32]);

        let mut above = CURVE_ORDER;
        above[31] += 1;
        reduce_mod_curve_order(&mut above);
        let mut one = [0u8; 32];
        one[31] = 1;
        assert_eq!(above, one);
    }

    #[test]
    fn malformed_responses_are_rejected() {
        let secp = Secp256k1::new();
        let secret_key = SecretKey::from_slice(&[1u8; 32]).unwrap();
        let public_key = PublicKey::from_secret_key(&secp, &secret_key);
        let message = Message::from_digest([2u8; 32]);
        let entropy = [3u8; 32];

        assert_eq!(
            verify(&secp, &public_key, &message, &entropy, &[0u8; 32], &[], 1,),
            Err(VerifyError::InvalidSignerCommitment)
        );

        assert_eq!(
            verify(
                &secp,
                &public_key,
                &message,
                &entropy,
                &public_key.serialize(),
                &[],
                1,
            ),
            Err(VerifyError::InvalidSignature)
        );
    }
}
