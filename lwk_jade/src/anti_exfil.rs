//! Temporary host-side implementation of the ECDSA anti-exfil protocol.
//!
//! The construction matches libsecp256k1-zkp's `ecdsa_s2c` module:
//! <https://github.com/BlockstreamResearch/secp256k1-zkp/blob/master/include/secp256k1_ecdsa_s2c.h>
//!
//! Replace this module with rust-secp256k1-zkp's ECDSA anti-exfil bindings once available:
//! <https://github.com/BlockstreamResearch/rust-secp256k1-zkp/issues/100>

use elements::{
    bitcoin::sign_message::{signed_msg_hash, MessageSignature},
    hashes::Hash,
    secp256k1_zkp::{ecdsa::Signature, Message, PublicKey, Scalar, Secp256k1, Verification},
};
use rand::RngCore;

use crate::{sign_message, SECP};

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
    verify_signature(
        secp,
        public_key,
        message,
        host_entropy,
        &opening,
        &signature,
    )
}

pub(crate) fn verify_compact<C: Verification>(
    secp: &Secp256k1<C>,
    public_key: &PublicKey,
    message: &Message,
    host_entropy: &[u8; 32],
    signer_commitment: &[u8],
    signature: &[u8],
) -> Result<(), VerifyError> {
    let opening = PublicKey::from_slice(signer_commitment)
        .map_err(|_| VerifyError::InvalidSignerCommitment)?;
    let signature =
        Signature::from_compact(signature).map_err(|_| VerifyError::InvalidSignature)?;
    verify_signature(
        secp,
        public_key,
        message,
        host_entropy,
        &opening,
        &signature,
    )
}

pub(crate) fn verify_message(
    public_key: &PublicKey,
    message: &str,
    host_entropy: &[u8; 32],
    signer_commitment: &[u8],
    encoded_signature: &str,
) -> Result<MessageSignature, VerifyError> {
    let digest = signed_msg_hash(message);
    let message = Message::from_digest(digest.to_byte_array());
    let signature = sign_message::parse(public_key, &message, encoded_signature)
        .ok_or(VerifyError::InvalidSignature)?;

    verify_compact(
        &SECP,
        public_key,
        &message,
        host_entropy,
        signer_commitment,
        &signature.compact,
    )?;
    Ok(signature.signature)
}

fn verify_signature<C: Verification>(
    secp: &Secp256k1<C>,
    public_key: &PublicKey,
    message: &Message,
    host_entropy: &[u8; 32],
    opening: &PublicKey,
    signature: &Signature,
) -> Result<(), VerifyError> {
    secp.verify_ecdsa(message, signature, public_key)
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
    use elements::secp256k1_zkp::{ecdsa::Signature, Message, PublicKey, Secp256k1};

    use super::{
        host_commitment, reduce_mod_curve_order, verify, verify_compact, VerifyError, CURVE_ORDER,
    };

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
    fn libwally_compatibility() {
        let d = |s: &str| hex::decode(s).unwrap();

        // Generated with libwally-core's Python binding:
        //
        // import wallycore as wally
        //
        // entropy = wally.hex_to_bytes("11" * 32)
        // priv_key = wally.hex_to_bytes("22" * 32)
        // msg = wally.hex_to_bytes("33" * 32)
        // flags = wally.EC_FLAG_ECDSA
        //
        // pub_key = wally.ec_public_key_from_private_key(priv_key)
        // host_commitment = wally.ae_host_commit_from_bytes(entropy, flags)
        // signer_commitment = wally.ae_signer_commit_from_bytes(
        //     priv_key, msg, host_commitment, flags
        // )
        // sig = wally.ae_sig_from_bytes(priv_key, msg, entropy, flags)
        // wally.ae_verify(pub_key, msg, entropy, signer_commitment, flags, sig)
        // der_sig = wally.ec_sig_to_der(sig)
        let host_entropy = [0x11; 32];
        let public_key = PublicKey::from_slice(&d(
            "02466d7fcae563e5cb09a0d1870bb580344804617879a14949cf22285f1bae3f27",
        ))
        .unwrap();
        let message = Message::from_digest([0x33; 32]);
        let signer_commitment =
            d("038e312c526bd1c2fa2cfe3b782f4cb1c6e6b9dc436236438aa2c543f6aee1a967");
        let mut signature = d(
            "3045022100dfa7fbbfec43a7ded916639981615c11f5e0cf2755099eebb85c733252206e4f022033100dff275df0a9fc6020fa4d63491c8919a47f1b4903b2519dec88f77ae23d",
        );
        signature.push(1);

        assert_eq!(
            hex::encode(host_commitment(&host_entropy)),
            "e231c06f3039640f06dbbe754e1914e3dfdc73d4caefaa3f7b6b692e026a328a"
        );

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

        let compact_signature = Signature::from_der(&signature[..signature.len() - 1])
            .unwrap()
            .serialize_compact();
        assert_eq!(
            verify_compact(
                &Secp256k1::verification_only(),
                &public_key,
                &message,
                &host_entropy,
                &signer_commitment,
                &compact_signature,
            ),
            Ok(())
        );

        let mut invalid_signer_commitment = signer_commitment.clone();
        invalid_signer_commitment[0] = 0x04;

        assert_eq!(
            verify(
                &Secp256k1::verification_only(),
                &public_key,
                &message,
                &host_entropy,
                &invalid_signer_commitment,
                &signature,
                1,
            ),
            Err(VerifyError::InvalidSignerCommitment)
        );

        let mut invalid_signature = signature.clone();
        invalid_signature[0] = 0x31;

        assert_eq!(
            verify(
                &Secp256k1::verification_only(),
                &public_key,
                &message,
                &host_entropy,
                &signer_commitment,
                &invalid_signature,
                1,
            ),
            Err(VerifyError::InvalidSignature)
        );
    }
}
