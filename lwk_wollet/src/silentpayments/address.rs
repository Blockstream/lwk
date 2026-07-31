//! Bech32m silent-payment addresses.

use bech32::primitives::decode::CheckedHrpstring;
use bech32::{Bech32m, Fe32, Hrp};

use crate::secp256k1::PublicKey;
use crate::Network;

/// Receiver scan and spend public keys.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SilentPaymentAddress {
    /// `B_scan` — used by the sender to compute the ECDH shared secret.
    pub scan: PublicKey,
    /// `B_spend` — base point the per-output spend key is tweaked from.
    pub spend: PublicKey,
}

impl SilentPaymentAddress {
    /// Version 0 (`q`).
    const VERSION: Fe32 = Fe32::Q;

    /// The expected payload size: two compressed secp256k1 public keys.
    const PAYLOAD_LEN: usize = 66;

    /// Network-specific Liquid address HRP.
    fn hrp(network: Network) -> Hrp {
        match network {
            Network::Liquid => Hrp::parse_unchecked("lqsp"),
            _ => Hrp::parse_unchecked("tlqsp"),
        }
    }

    /// Encodes for `network`.
    pub fn encode(&self, network: Network) -> String {
        use bech32::primitives::iter::{ByteIterExt, Fe32IterExt};

        let mut payload = Vec::with_capacity(Self::PAYLOAD_LEN);
        payload.extend_from_slice(&self.scan.serialize());
        payload.extend_from_slice(&self.spend.serialize());

        std::iter::once(Self::VERSION)
            .chain(payload.into_iter().bytes_to_fes())
            .with_checksum::<Bech32m>(&Self::hrp(network))
            .chars()
            .collect()
    }

    /// Parse a bech32m silent payment address, validating the HRP against `network`.
    pub fn parse(s: &str, network: Network) -> Result<Self, SilentPaymentAddressError> {
        use bech32::primitives::iter::Fe32IterExt;

        let checked = CheckedHrpstring::new::<Bech32m>(s)
            .map_err(|_| SilentPaymentAddressError::InvalidBech32m)?;

        if checked.hrp() != Self::hrp(network) {
            return Err(SilentPaymentAddressError::WrongNetwork);
        }

        let mut iter = checked.fe32_iter::<std::vec::IntoIter<u8>>();
        let version = iter.next().ok_or(SilentPaymentAddressError::Truncated)?;
        if version != Self::VERSION {
            return Err(SilentPaymentAddressError::UnknownVersion);
        }

        let bytes: Vec<u8> = iter.fes_to_bytes().collect();
        if bytes.len() != Self::PAYLOAD_LEN {
            return Err(SilentPaymentAddressError::WrongPayloadLength(bytes.len()));
        }
        let scan = PublicKey::from_slice(&bytes[..33])
            .map_err(|_| SilentPaymentAddressError::InvalidPublicKey)?;
        let spend = PublicKey::from_slice(&bytes[33..])
            .map_err(|_| SilentPaymentAddressError::InvalidPublicKey)?;
        Ok(SilentPaymentAddress { scan, spend })
    }
}

/// Errors parsing a [`SilentPaymentAddress`].
#[derive(thiserror::Error, Debug, Clone, Copy, PartialEq, Eq)]
pub enum SilentPaymentAddressError {
    /// Not a valid bech32m string.
    #[error("not a valid bech32m string")]
    InvalidBech32m,

    /// HRP does not match the expected network.
    #[error("address HRP does not match network")]
    WrongNetwork,

    /// Payload ended before the version/keys could be read.
    #[error("address payload truncated")]
    Truncated,

    /// Address version is not supported.
    #[error("unsupported silent payment address version")]
    UnknownVersion,

    /// Payload is not the expected 66 bytes (two compressed pubkeys).
    #[error("expected 66-byte payload, got {0}")]
    WrongPayloadLength(usize),

    /// A public key in the payload is not a valid point.
    #[error("invalid public key in address payload")]
    InvalidPublicKey,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::silentpayments::test_fixture::SilentPaymentTestData as Data;
    use crate::silentpayments::SilentPaymentScan;

    #[test]
    fn address_encodes_and_round_trips_per_network() {
        let address = Data::material(0x11, 0x22).address();

        for (network, hrp) in [
            (Network::Liquid, "lqsp1"),
            (Network::TestnetLiquid, "tlqsp1"),
            (Network::default_regtest(), "tlqsp1"),
        ] {
            let encoded = address.encode(network);
            assert!(
                encoded.starts_with(hrp),
                "address {encoded} should start with {hrp}"
            );
            assert_eq!(
                SilentPaymentAddress::parse(&encoded, network).unwrap(),
                address,
                "address did not round-trip on {network:?}"
            );
        }

        assert_eq!(
            address.encode(Network::TestnetLiquid),
            address.encode(Network::default_regtest()),
            "testnet and regtest must share the tlqsp HRP"
        );
    }

    #[test]
    fn address_rejects_wrong_network_and_garbage() {
        let address = Data::material(0x11, 0x22).address();
        let mainnet = address.encode(Network::Liquid);

        for network in [Network::TestnetLiquid, Network::default_regtest()] {
            assert_eq!(
                SilentPaymentAddress::parse(&mainnet, network),
                Err(SilentPaymentAddressError::WrongNetwork),
                "a mainnet address must not parse as {network:?}"
            );
        }

        assert!(SilentPaymentAddress::parse("not an address", Network::Liquid).is_err());
        // Valid bech32m but wrong payload length (no key bytes).
        assert!(SilentPaymentAddress::parse("lq1qqqqqq", Network::Liquid).is_err());
    }
}
