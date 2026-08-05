use std::fmt::Display;
use std::str::FromStr;

use bech32::primitives::decode::CheckedHrpstring;
use bech32::{Bech32m, ByteIterExt, Fe32, Fe32IterExt, Hrp};
use elements::secp256k1_zkp::PublicKey;
use lwk_common::Network;

use super::SilentPaymentError;

/// Human readable part of a silent payment address on Liquid
const HRP_LIQUID: &str = "lqsp";

/// Human readable part of a silent payment address on Liquid testnet
const HRP_TESTNET: &str = "tlqsp";

/// Human readable part of a silent payment address on Elements regtest
const HRP_REGTEST: &str = "elsp";

/// The only address version currently defined
const VERSION_0: Fe32 = Fe32::Q;

/// Version reserved by BIP352 to signal a backward incompatible change
const VERSION_RESERVED: Fe32 = Fe32::L;

/// Length of the address payload: two compressed public keys
const PAYLOAD_LEN: usize = 66;

/// The network a [`SilentPaymentAddress`] belongs to.
///
/// Unlike [`Network`] it only distinguishes what is actually encoded in the address, so that
/// parsing and re-encoding an address is lossless.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SilentPaymentNetwork {
    /// Liquid mainnet, addresses start with "lqsp1"
    Liquid,

    /// Liquid testnet, addresses start with "tlqsp1"
    LiquidTestnet,

    /// Elements regtest, addresses start with "elsp1"
    ElementsRegtest,
}

impl SilentPaymentNetwork {
    fn hrp(&self) -> Hrp {
        let s = match self {
            SilentPaymentNetwork::Liquid => HRP_LIQUID,
            SilentPaymentNetwork::LiquidTestnet => HRP_TESTNET,
            SilentPaymentNetwork::ElementsRegtest => HRP_REGTEST,
        };
        Hrp::parse_unchecked(s)
    }

    fn from_hrp(hrp: &Hrp) -> Option<Self> {
        match hrp.to_lowercase().as_str() {
            HRP_LIQUID => Some(SilentPaymentNetwork::Liquid),
            HRP_TESTNET => Some(SilentPaymentNetwork::LiquidTestnet),
            HRP_REGTEST => Some(SilentPaymentNetwork::ElementsRegtest),
            _ => None,
        }
    }
}

impl From<Network> for SilentPaymentNetwork {
    fn from(network: Network) -> Self {
        match network {
            Network::Liquid => SilentPaymentNetwork::Liquid,
            Network::TestnetLiquid => SilentPaymentNetwork::LiquidTestnet,
            Network::CustomElements(_) => SilentPaymentNetwork::ElementsRegtest,
        }
    }
}

/// A silent payment address, a static address that can be published without losing privacy.
///
/// It encodes the scan public key and the spend public key of the receiver, senders derive a
/// fresh confidential taproot output from those keys and from their own transaction inputs, so
/// that two payments to the same address are unlinkable on chain.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SilentPaymentAddress {
    version: Fe32,
    scan_public_key: PublicKey,
    spend_public_key: PublicKey,
    network: SilentPaymentNetwork,
}

impl SilentPaymentAddress {
    /// Create a version 0 silent payment address
    pub fn new(
        scan_public_key: PublicKey,
        spend_public_key: PublicKey,
        network: SilentPaymentNetwork,
    ) -> Self {
        Self {
            version: VERSION_0,
            scan_public_key,
            spend_public_key,
            network,
        }
    }

    /// The key the receiver uses to detect payments, its secret counterpart is needed to scan
    /// the blockchain
    pub fn scan_public_key(&self) -> PublicKey {
        self.scan_public_key
    }

    /// The key the outputs are derived from, its secret counterpart is needed to spend the
    /// received funds
    pub fn spend_public_key(&self) -> PublicKey {
        self.spend_public_key
    }

    /// The network this address belongs to
    pub fn network(&self) -> SilentPaymentNetwork {
        self.network
    }

    /// Whether this address can be paid on the given network
    pub fn is_for_network(&self, network: Network) -> bool {
        self.network == network.into()
    }

    /// Encode with an arbitrary human readable part, the BIP352 encoding is shared with
    /// Bitcoin so this allows to check our codec against the BIP352 test vectors
    pub(super) fn encode_with_hrp(&self, hrp: Hrp) -> String {
        let mut payload = [0u8; PAYLOAD_LEN];
        payload[..33].copy_from_slice(&self.scan_public_key.serialize());
        payload[33..].copy_from_slice(&self.spend_public_key.serialize());

        payload
            .iter()
            .copied()
            .bytes_to_fes()
            .with_checksum::<Bech32m>(&hrp)
            .with_witness_version(self.version)
            .chars()
            .collect()
    }

    /// Decode without checking the human readable part, see [`Self::encode_with_hrp`]
    pub(super) fn decode_with_hrp(
        s: &str,
    ) -> Result<(Hrp, Fe32, PublicKey, PublicKey), SilentPaymentError> {
        let mut parsed = CheckedHrpstring::new::<Bech32m>(s)
            .map_err(|e| SilentPaymentError::AddressEncoding(e.to_string()))?;
        let version = parsed
            .remove_witness_version()
            .ok_or_else(|| SilentPaymentError::AddressEncoding("missing version".into()))?;
        if version == VERSION_RESERVED {
            return Err(SilentPaymentError::AddressVersion(version.to_char()));
        }
        let payload: Vec<u8> = parsed.byte_iter().collect();
        // a future version may append data after the keys, which stay at the beginning
        let expected_len = if version == VERSION_0 {
            payload.len() == PAYLOAD_LEN
        } else {
            payload.len() >= PAYLOAD_LEN
        };
        if !expected_len {
            return Err(SilentPaymentError::AddressLength(payload.len()));
        }
        let scan = PublicKey::from_slice(&payload[..33])?;
        let spend = PublicKey::from_slice(&payload[33..PAYLOAD_LEN])?;
        Ok((parsed.hrp(), version, scan, spend))
    }
}

impl Display for SilentPaymentAddress {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.encode_with_hrp(self.network.hrp()))
    }
}

impl FromStr for SilentPaymentAddress {
    type Err = crate::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (hrp, version, scan_public_key, spend_public_key) = Self::decode_with_hrp(s)?;
        let network = SilentPaymentNetwork::from_hrp(&hrp)
            .ok_or_else(|| SilentPaymentError::AddressHrp(hrp.to_lowercase()))?;
        Ok(Self {
            version,
            scan_public_key,
            spend_public_key,
            network,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::silent_payments::test_vectors::test_vectors;

    fn address() -> SilentPaymentAddress {
        let scan = "0220bcfac5b99e04ad1a06ddfb016ee13582609d60b6291e98d01a9bc9a16c96d4";
        let spend = "025cc9856d6f8375350e123978daac200c260cb5b5ae83106cab90484dcd8fcf36";
        SilentPaymentAddress::new(
            scan.parse().unwrap(),
            spend.parse().unwrap(),
            SilentPaymentNetwork::Liquid,
        )
    }

    #[test]
    fn address_roundtrip() {
        let address = address();
        let encoded = address.to_string();
        assert!(encoded.starts_with("lqsp1q"), "{encoded}");
        assert_eq!(encoded.parse::<SilentPaymentAddress>().unwrap(), address);
        assert_eq!(
            address.scan_public_key().to_string(),
            "0220bcfac5b99e04ad1a06ddfb016ee13582609d60b6291e98d01a9bc9a16c96d4"
        );

        for (network, prefix) in [
            (SilentPaymentNetwork::LiquidTestnet, "tlqsp1q"),
            (SilentPaymentNetwork::ElementsRegtest, "elsp1q"),
        ] {
            let a = SilentPaymentAddress::new(
                address.scan_public_key(),
                address.spend_public_key(),
                network,
            );
            assert!(a.to_string().starts_with(prefix), "{a}");
            assert_eq!(a.to_string().parse::<SilentPaymentAddress>().unwrap(), a);
        }
    }

    #[test]
    fn address_network() {
        let address = address();
        assert!(address.is_for_network(Network::Liquid));
        assert!(!address.is_for_network(Network::TestnetLiquid));
        assert!(!address.is_for_network(Network::default_regtest()));
    }

    #[test]
    fn address_invalid() {
        // a bitcoin silent payment address must not be accepted on liquid
        let bitcoin = "sp1qqgste7k9hx0qftg6qmwlkqtwuy6cycyavzmzj85c6qdfhjdpdjtdgqjuexzk6murw56suy3e0rd2cgqvycxttddwsvgxe2usfpxumr70xc9pkqwv";
        assert!(bitcoin.parse::<SilentPaymentAddress>().is_err());

        let mut wrong_checksum = address().to_string();
        wrong_checksum.pop();
        wrong_checksum.push('q');
        assert!(wrong_checksum.parse::<SilentPaymentAddress>().is_err());

        let truncated = &address().to_string()[..50];
        assert!(truncated.parse::<SilentPaymentAddress>().is_err());
    }

    /// The encoding is the same as BIP352, only the human readable part differs, so the
    /// addresses in the BIP352 test vectors must round trip through our codec.
    #[test]
    fn bip352_address_encoding() {
        use crate::silent_payments::test_vectors::{encode_bip352_address, parse_bip352_address};

        let mut checked = 0;
        for vector in test_vectors() {
            for receiving in &vector.receiving {
                for address in &receiving.expected.addresses {
                    let parsed = parse_bip352_address(address);
                    assert_eq!(&encode_bip352_address(&parsed), address);
                    checked += 1;
                }
            }
        }
        assert_eq!(checked, 44);
    }
}
