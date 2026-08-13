use elements::{
    bitcoin::{
        base64::prelude::{Engine as _, BASE64_STANDARD},
        secp256k1::ecdsa::{RecoverableSignature, RecoveryId},
        sign_message::MessageSignature,
    },
    secp256k1_zkp::{Message, PublicKey},
};

use crate::SECP;

pub(crate) struct ParsedMessageSignature {
    pub(crate) signature: MessageSignature,
    pub(crate) compact: [u8; 64],
}

/// Parse a base64 signature returned by either Jade message-signing flow.
///
/// Legacy signing returns a 65-byte Bitcoin message signature containing a recovery header and
/// the 64-byte compact signature. Anti-exfil signing returns only the compact signature because
/// the sign-to-contract operation does not produce a recovery ID. For the latter, try each
/// possible recovery ID and retain the one that recovers the expected Jade public key.
/// See <https://github.com/Blockstream/Jade/blob/1f2e4403b351bec2547c780ada1c958a51f74537/main/wallet.c#L1478-L1514>.
pub(crate) fn parse(
    public_key: &PublicKey,
    message: &Message,
    encoded_signature: &str,
) -> Option<ParsedMessageSignature> {
    let signature_bytes = BASE64_STANDARD.decode(encoded_signature).ok()?;
    let compact: [u8; 64] = match signature_bytes.len() {
        64 => signature_bytes.as_slice(),
        65 => &signature_bytes[1..],
        _ => return None,
    }
    .try_into()
    .ok()?;
    let (signature, recovered) = if signature_bytes.len() == 65 {
        let signature = MessageSignature::from_slice(&signature_bytes).ok()?;
        let recovered = SECP.recover_ecdsa(message, &signature.signature).ok()?;
        (signature, recovered)
    } else {
        let (signature, recovered) = (0..=3)
            .filter_map(|id| RecoveryId::from_i32(id).ok())
            .filter_map(|id| RecoverableSignature::from_compact(&compact, id).ok())
            .find_map(|signature| {
                let recovered = SECP.recover_ecdsa(message, &signature).ok()?;
                (recovered == *public_key).then_some((signature, recovered))
            })?;
        (
            MessageSignature {
                signature,
                compressed: true,
            },
            recovered,
        )
    };
    if recovered != *public_key {
        return None;
    }

    Some(ParsedMessageSignature { signature, compact })
}

#[cfg(test)]
mod tests {
    use elements::{
        bitcoin::{
            base64::prelude::{Engine as _, BASE64_STANDARD},
            sign_message::{signed_msg_hash, MessageSignature},
        },
        hashes::Hash,
        secp256k1_zkp::{Message, PublicKey, Secp256k1, SecretKey},
    };

    use super::parse;

    #[test]
    fn parse_message_signature_formats() {
        let secp = Secp256k1::new();
        let secret_key = SecretKey::from_slice(&[0x22; 32]).unwrap();
        let public_key = PublicKey::from_secret_key(&secp, &secret_key);
        let message = "Hello world!";
        let digest = signed_msg_hash(message);
        let secp_message = Message::from_digest(digest.to_byte_array());
        let signature = secp.sign_ecdsa_recoverable(&secp_message, &secret_key);
        let message_signature = MessageSignature {
            signature,
            compressed: true,
        };

        let parsed = parse(&public_key, &secp_message, &message_signature.to_base64()).unwrap();
        assert_eq!(parsed.signature, message_signature);

        let (_, compact) = signature.serialize_compact();
        let parsed = parse(&public_key, &secp_message, &BASE64_STANDARD.encode(compact)).unwrap();
        assert_eq!(parsed.signature, message_signature);
        assert_eq!(parsed.compact, compact);

        let other_key = SecretKey::from_slice(&[0x23; 32]).unwrap();
        let other_key = PublicKey::from_secret_key(&secp, &other_key);
        assert!(parse(&other_key, &secp_message, &BASE64_STANDARD.encode(compact)).is_none());
    }
}
