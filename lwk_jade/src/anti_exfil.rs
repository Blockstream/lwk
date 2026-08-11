//! Temporary host-side implementation of the ECDSA anti-exfil protocol.
//!
//! The construction matches libsecp256k1-zkp's `ecdsa_s2c` module:
//! <https://github.com/BlockstreamResearch/secp256k1-zkp/blob/master/include/secp256k1_ecdsa_s2c.h>
//!
//! Replace this module with rust-secp256k1-zkp's ECDSA anti-exfil bindings once available:
//! <https://github.com/BlockstreamResearch/rust-secp256k1-zkp/issues/100>

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
        // Generated with libwally-core's Python binding:
        // wally.ae_host_commit_from_bytes(entropy, wally.EC_FLAG_ECDSA)
        // https://github.com/ElementsProject/libwally-core/blob/3bf543cd06a67fdd877688a6304808f270351aee/src/pyexample/anti-exfil.py#L9-L10
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

        let mut max = [u8::MAX; 32];
        reduce_mod_curve_order(&mut max);
        assert_eq!(
            hex::encode(max),
            "000000000000000000000000000000014551231950b75fc4402da1732fc9bebe"
        );
    }

    #[test]
    fn valid_signature_is_accepted() {
        // Generated with secp256k1-zkp 0.10.1's anti-exfil APIs using a 0x55 secret
        // key, 0x88 message, and 0x42 host entropy.
        let public_key = PublicKey::from_slice(
            &hex::decode("029ac20335eb38768d2052be1dbbc3c8f6178407458e51e6b4ad22f1d91758895b")
                .unwrap(),
        )
        .unwrap();
        let message = Message::from_digest([0x88; 32]);
        let host_entropy = [0x42; 32];
        let signer_commitment =
            hex::decode("03de63785e2b5f823b076935bd7877fd8f03f678b7ec42e14779c5e34a9a109a12")
                .unwrap();
        let signature = hex::decode(
            "304402207ab9c455903c04a4ed018a2168020ba1d6013629dcdb626120511641ee0db33c02205339e6a49be83aeb4b27da51d712f1a74899e5435606a33b301e501d6bc064e601",
        )
        .unwrap();

        assert_eq!(
            verify(
                &Secp256k1::verification_only(),
                &public_key,
                &message,
                &host_entropy,
                &signer_commitment,
                &signature,
                1,
            ),
            Ok(())
        );
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
