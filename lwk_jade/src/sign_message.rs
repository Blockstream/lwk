use elements::{
    bitcoin::{
        base64::prelude::{Engine as _, BASE64_STANDARD},
        secp256k1::ecdsa::{RecoverableSignature, RecoveryId},
        sign_message::{signed_msg_hash, MessageSignature},
    },
    hashes::Hash,
    secp256k1_zkp::{Message, PublicKey, Secp256k1},
};

pub(crate) struct ParsedMessageSignature {
    pub(crate) signature: MessageSignature,
    #[allow(dead_code)] // Used by anti-exfil verification in the following commit.
    pub(crate) compact: [u8; 64],
}

/// Parse a base64 signature returned by either Jade message-signing flow.
///
/// Legacy signing returns a 65-byte Bitcoin message signature containing a recovery header and
/// the 64-byte compact signature. Anti-exfil signing returns only the compact signature because
/// the signer-to-contract operation does not produce a recovery ID. For the latter, try each
/// possible recovery ID and retain the one that recovers the expected Jade public key.
/// See <https://github.com/Blockstream/Jade/blob/1f2e4403b351bec2547c780ada1c958a51f74537/main/wallet.c#L1478-L1514>.
pub(crate) fn parse(
    public_key: &PublicKey,
    message: &str,
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
    let digest = signed_msg_hash(message);
    let message = Message::from_digest(digest.to_byte_array());
    let secp = Secp256k1::verification_only();

    let signature = if signature_bytes.len() == 65 {
        MessageSignature::from_slice(&signature_bytes).ok()?
    } else {
        let signature = (0..=3)
            .filter_map(|id| RecoveryId::from_i32(id).ok())
            .filter_map(|id| RecoverableSignature::from_compact(&compact, id).ok())
            .find(|signature| {
                secp.recover_ecdsa(&message, signature)
                    .is_ok_and(|recovered| recovered == *public_key)
            })?;
        MessageSignature {
            signature,
            compressed: true,
        }
    };
    let recovered = signature.recover_pubkey(&secp, digest).ok()?;
    if recovered.inner != *public_key {
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
        let signature =
            secp.sign_ecdsa_recoverable(&Message::from_digest(digest.to_byte_array()), &secret_key);
        let message_signature = MessageSignature {
            signature,
            compressed: true,
        };

        let parsed = parse(&public_key, message, &message_signature.to_base64()).unwrap();
        assert_eq!(parsed.signature, message_signature);

        let (_, compact) = signature.serialize_compact();
        let parsed = parse(&public_key, message, &BASE64_STANDARD.encode(compact)).unwrap();
        assert_eq!(parsed.signature, message_signature);
        assert_eq!(parsed.compact, compact);

        let other_key = SecretKey::from_slice(&[0x23; 32]).unwrap();
        let other_key = PublicKey::from_secret_key(&secp, &other_key);
        assert!(parse(&other_key, message, &BASE64_STANDARD.encode(compact)).is_none());
    }
}
