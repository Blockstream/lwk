//! Ephemeral Taproot pubkey and address generator for argument-bound programs.
//!
//! Produces a deterministic X-only public key and corresponding address without
//! holding a private key, based on a random seed. The resulting trio
//! `<seed_or_ext-xonly_hex>:<pubkey_hex>:<taproot_address>` can be printed and
//! later verified with the same arguments to prevent mismatches.
//!
//! Identity field formats:
//! - `seed_hex`: 32-byte random seed (legacy/current default)
//! - `ext-<xonly_hex>`: externally supplied 32-byte x-only key handle

use crate::error::ProgramError;
use crate::simplicityhl::elements::{schnorr::XOnlyPublicKey, Address};
use crate::simplicityhl::simplicity::bitcoin::key::Parity;
use crate::simplicityhl::simplicity::bitcoin::PublicKey;
use crate::simplicityhl::simplicity::ToXOnlyPubkey;

use std::fmt::Display;
use std::str::FromStr;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use lwk_common::Network;
use lwk_signer::bip39::rand::{thread_rng, RngCore};

use lwk_wollet::elements::hex::ToHex;
use lwk_wollet::hashes::hex::FromHex;
use lwk_wollet::hashes::{sha256, Hash, HashEngine};

/// Errors from taproot pubkey generation and verification.
#[derive(Debug, thiserror::Error)]
pub enum TaprootPubkeyGenError {
    #[error("Invalid pubkey recovered: expected {expected}, got {actual}")]
    InvalidPubkey { expected: String, actual: String },

    #[error("Invalid address recovered: expected {expected}, got {actual}")]
    InvalidAddress { expected: String, actual: String },

    #[error(
        "Invalid taproot pubkey gen string: expected 3 parts separated by ':', got {parts_count}"
    )]
    InvalidFormat { parts_count: usize },

    #[error("Invalid seed length: expected 32 bytes, got {actual}")]
    InvalidSeedLength { actual: usize },

    #[error("Failed to parse public key: {0}")]
    PublicKeyParse(#[from] simplicityhl::simplicity::bitcoin::key::ParsePublicKeyError),

    #[error("Failed to parse address: {0}")]
    AddressParse(#[from] simplicityhl::elements::address::AddressError),

    #[error("Failed to create X-only public key from bytes: {0}")]
    XOnlyPublicKey(#[from] simplicityhl::simplicity::bitcoin::secp256k1::Error),

    #[error("Invalid external x-only key: {0}")]
    InvalidExternalKey(String),

    #[error("Failed to generate address: {0}")]
    AddressGeneration(#[from] ProgramError),

    #[error("hex error: {0}")]
    Hex(#[from] lwk_wollet::hashes::hex::HexToBytesError),
}

/// Generate a valid ephemeral public key and its seed; repeats until valid.
pub fn generate_public_key_without_private() -> (PublicKey, Vec<u8>) {
    let derived_public_key;
    loop {
        if let Ok(public_key) = try_generate_public_key_without_private() {
            derived_public_key = public_key;
            break;
        }
    }

    derived_public_key
}

/// System-random 32-byte seed.
///
/// # Panics
/// Panics if the system random number generator fails.
pub fn get_random_seed() -> [u8; 32] {
    let mut bytes: [u8; 32] = [0; 32];
    thread_rng().fill_bytes(&mut bytes);
    bytes
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
enum TaprootIdentity {
    Seed(Vec<u8>),
    ExternalXOnly(XOnlyPublicKey),
}

/// Container for the seed, public key and derived address.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TaprootPubkeyGen {
    identity: TaprootIdentity,
    pub pubkey: PublicKey,
    pub address: Address,
}

impl TaprootPubkeyGen {
    /// Build from current process randomness and compute the address given `arguments`.
    ///
    /// Kept as `from` for compatibility with existing callers.
    ///
    /// # Errors
    /// Returns error if address generation fails.
    pub fn from<A>(
        arguments: &A,
        network: Network,
        get_address: &impl Fn(&XOnlyPublicKey, &A, Network) -> Result<Address, ProgramError>,
    ) -> Result<Self, TaprootPubkeyGenError> {
        let (derived_public_key, seed) = generate_public_key_without_private();

        let address = get_address(&derived_public_key.to_x_only_pubkey(), arguments, network)?;

        Ok(Self {
            identity: TaprootIdentity::Seed(seed),
            pubkey: derived_public_key,
            address,
        })
    }

    /// Parse from string and verify that pubkey and address match the provided arguments.
    ///
    /// # Errors
    /// Returns error if parsing fails or verification doesn't match.
    pub fn build_from_str<A>(
        s: &str,
        arguments: &A,
        network: Network,
        get_address: &impl Fn(&XOnlyPublicKey, &A, Network) -> Result<Address, ProgramError>,
    ) -> Result<Self, TaprootPubkeyGenError> {
        let taproot_pubkey_gen = Self::parse_from_str(s)?;

        taproot_pubkey_gen.verify(arguments, network, get_address)?;

        Ok(taproot_pubkey_gen)
    }

    /// Verify that the stored pubkey and address are consistent with `arguments`.
    ///
    /// # Errors
    /// Returns error if pubkey or address doesn't match the expected values.
    pub fn verify<A>(
        &self,
        arguments: &A,
        network: Network,
        get_address: &impl Fn(&XOnlyPublicKey, &A, Network) -> Result<Address, ProgramError>,
    ) -> Result<(), TaprootPubkeyGenError> {
        match &self.identity {
            TaprootIdentity::Seed(seed) => {
                let rand_seed = seed.as_slice();

                let mut eng = sha256::Hash::engine();
                eng.input(rand_seed);
                eng.input(rand_seed);
                eng.input(rand_seed);
                let potential_pubkey: [u8; 32] = sha256::Hash::from_engine(eng).to_byte_array();

                let expected_pubkey: PublicKey = XOnlyPublicKey::from_slice(&potential_pubkey)?
                    .public_key(Parity::Even)
                    .into();

                if expected_pubkey != self.pubkey {
                    return Err(TaprootPubkeyGenError::InvalidPubkey {
                        expected: expected_pubkey.to_string(),
                        actual: self.pubkey.to_string(),
                    });
                }
            }
            TaprootIdentity::ExternalXOnly(xonly) => {
                if &self.pubkey.to_x_only_pubkey() != xonly {
                    let expected_pubkey: PublicKey = xonly.public_key(Parity::Even).into();
                    return Err(TaprootPubkeyGenError::InvalidPubkey {
                        expected: expected_pubkey.to_string(),
                        actual: self.pubkey.to_string(),
                    });
                }
            }
        }

        let expected_address = get_address(&self.pubkey.to_x_only_pubkey(), arguments, network)?;
        if self.address != expected_address {
            return Err(TaprootPubkeyGenError::InvalidAddress {
                expected: expected_address.to_string(),
                actual: self.address.to_string(),
            });
        }

        Ok(())
    }

    /// Get the X-only public key.
    pub fn get_x_only_pubkey(&self) -> XOnlyPublicKey {
        self.pubkey.to_x_only_pubkey()
    }

    /// Serializes the structure into JSON
    ///
    /// # Errors
    ///
    /// Returns an error when arguments serialization failed
    pub fn to_json(&self) -> serde_json::Result<Value> {
        serde_json::to_value(self)
    }

    /// Parse `<seed_or_ext-xonly_hex>:<pubkey>:<address>` representation.
    fn parse_from_str(s: &str) -> Result<Self, TaprootPubkeyGenError> {
        let parts = s.split(':').collect::<Vec<&str>>();

        if parts.len() != 3 {
            return Err(TaprootPubkeyGenError::InvalidFormat {
                parts_count: parts.len(),
            });
        }

        let identity = if let Some(xonly_hex) = parts[0].strip_prefix("ext-") {
            let xonly_bytes = Vec::<u8>::from_hex(xonly_hex)?;
            TaprootIdentity::ExternalXOnly(
                XOnlyPublicKey::from_slice(&xonly_bytes)
                    .map_err(|e| TaprootPubkeyGenError::InvalidExternalKey(e.to_string()))?,
            )
        } else {
            let seed = Vec::<u8>::from_hex(parts[0])?;
            if seed.len() != 32 {
                return Err(TaprootPubkeyGenError::InvalidSeedLength { actual: seed.len() });
            }
            TaprootIdentity::Seed(seed)
        };

        Ok(Self {
            identity,
            pubkey: PublicKey::from_str(parts[1])?,
            address: Address::from_str(parts[2])?,
        })
    }
}

impl Display for TaprootPubkeyGen {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let id = match &self.identity {
            TaprootIdentity::Seed(seed) => seed.to_hex(),
            TaprootIdentity::ExternalXOnly(xonly) => {
                format!("ext-{}", xonly.serialize().to_hex())
            }
        };
        write!(f, "{}:{}:{}", id, self.pubkey, self.address)
    }
}

/// Try to deterministically map a random seed into a valid X-only pubkey.
///
/// Compatibility note:
/// candidate bytes are derived as `sha256(seed || seed || seed)`.
/// This is a deterministic mapping used by `wallet-abi-0.1`, not a standard KDF.
fn try_generate_public_key_without_private() -> Result<(PublicKey, Vec<u8>), TaprootPubkeyGenError>
{
    let rand_seed: [u8; 32] = get_random_seed();

    let mut eng = sha256::Hash::engine();
    eng.input(&rand_seed);
    eng.input(&rand_seed);
    eng.input(&rand_seed);
    let potential_pubkey: [u8; 32] = sha256::Hash::from_engine(eng).to_byte_array();

    Ok((
        XOnlyPublicKey::from_slice(&potential_pubkey)?
            .public_key(Parity::Even)
            .into(),
        rand_seed.to_vec(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXED_ADDRESS: &str =
        "lq1qqvxk052kf3qtkxmrakx50a9gc3smqad2ync54hzntjt980kfej9kkfe0247rp5h4yzmdftsahhw64uy8pzfe7cpg4fgykm7cv";
    const ALTERNATE_ADDRESS: &str =
        "lq1qqtmf5e3g4ats3yexwdfn6kfhp9sl68kdl47g75k58rvw2w33zuarwfe0247rp5h4yzmdftsahhw64uy8pzfe7k9s63c7cku58";

    fn fixed_address(_: &XOnlyPublicKey, _: &(), _: Network) -> Result<Address, ProgramError> {
        Ok(Address::from_str(FIXED_ADDRESS).expect("valid fixed address"))
    }

    #[test]
    fn roundtrip_display_and_build_from_str() {
        let generated = TaprootPubkeyGen::from(&(), Network::Liquid, &fixed_address)
            .expect("generate taproot handle");
        let serialized = generated.to_string();

        let rebuilt =
            TaprootPubkeyGen::build_from_str(&serialized, &(), Network::Liquid, &fixed_address)
                .expect("rebuild and verify");

        assert_eq!(rebuilt, generated);
    }

    #[test]
    fn invalid_format_is_rejected() {
        let err =
            TaprootPubkeyGen::build_from_str("qwerty:12345", &(), Network::Liquid, &fixed_address)
                .expect_err("invalid format must fail");
        assert!(matches!(
            err,
            TaprootPubkeyGenError::InvalidFormat { parts_count: 2 }
        ));
    }

    #[test]
    fn tampered_pubkey_is_rejected() {
        let generated = TaprootPubkeyGen::from(&(), Network::Liquid, &fixed_address)
            .expect("generate taproot handle");
        let mut parts = generated
            .to_string()
            .split(':')
            .map(ToOwned::to_owned)
            .collect::<Vec<_>>();
        parts[1] = "0279be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798".to_string();
        let tampered = parts.join(":");

        let err = TaprootPubkeyGen::build_from_str(&tampered, &(), Network::Liquid, &fixed_address)
            .expect_err("tampered pubkey must fail");
        assert!(matches!(err, TaprootPubkeyGenError::InvalidPubkey { .. }));
    }

    #[test]
    fn tampered_address_is_rejected() {
        let generated = TaprootPubkeyGen::from(&(), Network::Liquid, &fixed_address)
            .expect("generate taproot handle");
        let mut parts = generated
            .to_string()
            .split(':')
            .map(ToOwned::to_owned)
            .collect::<Vec<_>>();
        parts[2] = ALTERNATE_ADDRESS.to_string();
        let tampered = parts.join(":");

        let err = TaprootPubkeyGen::build_from_str(&tampered, &(), Network::Liquid, &fixed_address)
            .expect_err("tampered address must fail");
        assert!(matches!(err, TaprootPubkeyGenError::InvalidAddress { .. }));
    }
}
