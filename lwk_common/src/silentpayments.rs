//! Shared Liquid silent-payment boundary types.

use elements_miniscript::elements::bitcoin::bip32::{ChildNumber, DerivationPath};
use elements_miniscript::elements::bitcoin::secp256k1::{PublicKey, Scalar, SecretKey};
use elements_miniscript::elements::pset::raw::ProprietaryKey;
use elements_miniscript::elements::pset::Input as PsetInput;

use crate::Signer;

const PURPOSE: u32 = 352;

const COIN_TYPE_LIQUID_MAINNET: u32 = 1776;

const COIN_TYPE_LIQUID_TESTNET: u32 = 1;

const HARDENED_THRESHOLD: u32 = 0x8000_0000;

/// Errors for invalid hardened account coordinates.
#[derive(thiserror::Error, Debug, Clone, Copy, PartialEq, Eq)]
pub enum SilentPaymentAccountError {
    /// The coin type has bit 31 set, so `coin_type'` is not a valid BIP-32 index.
    #[error("silent payment coin type {0} cannot be hardened (must be < 2^31)")]
    CoinTypeNotHardenable(u32),

    /// The account index has bit 31 set, so `account'` is not a valid BIP-32 index.
    #[error("silent payment account {0} cannot be hardened (must be < 2^31)")]
    AccountNotHardenable(u32),
}

/// Silent-payment key derivation coordinates.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SilentPaymentAccount {
    coin_type: u32,
    account: u32,
}

impl SilentPaymentAccount {
    /// The account for Liquid mainnet at index `account`.
    pub fn liquid_mainnet(account: u32) -> Self {
        SilentPaymentAccount {
            coin_type: COIN_TYPE_LIQUID_MAINNET,
            account,
        }
    }

    /// The account for Liquid testnet/regtest at index `account`.
    pub fn liquid_testnet(account: u32) -> Self {
        SilentPaymentAccount {
            coin_type: COIN_TYPE_LIQUID_TESTNET,
            account,
        }
    }

    /// Builds an account from a coin type and account index.
    pub fn from_raw(coin_type: u32, account: u32) -> Result<Self, SilentPaymentAccountError> {
        if coin_type >= HARDENED_THRESHOLD {
            return Err(SilentPaymentAccountError::CoinTypeNotHardenable(coin_type));
        }
        if account >= HARDENED_THRESHOLD {
            return Err(SilentPaymentAccountError::AccountNotHardenable(account));
        }
        Ok(SilentPaymentAccount { coin_type, account })
    }

    /// The SLIP-44 coin type this account uses.
    pub fn coin_type(&self) -> u32 {
        self.coin_type
    }

    /// The account index.
    pub fn account(&self) -> u32 {
        self.account
    }

    /// Scan-key path: `m/352'/<coin>'/<account>'/1'/0`.
    pub fn scan_path(&self) -> DerivationPath {
        self.path_at(1)
    }

    /// Spend-key path: `m/352'/<coin>'/<account>'/0'/0`.
    pub fn spend_path(&self) -> DerivationPath {
        self.path_at(0)
    }

    fn path_at(&self, change: u32) -> DerivationPath {
        DerivationPath::from(vec![
            ChildNumber::from_hardened_idx(PURPOSE).expect("352 < 2^31"),
            ChildNumber::from_hardened_idx(self.coin_type).expect("checked < 2^31 at construction"),
            ChildNumber::from_hardened_idx(self.account).expect("checked < 2^31 at construction"),
            ChildNumber::from_hardened_idx(change).expect("0 or 1"),
            ChildNumber::from_normal_idx(0).expect("0 is always a valid normal index"),
        ])
    }
}

/// Scan-only material exported by a signer.
#[derive(Debug, Clone, Copy)]
pub struct SilentPaymentScanMaterial {
    account: SilentPaymentAccount,
    scan_seckey: SecretKey,
    spend_pubkey: PublicKey,
}

impl SilentPaymentScanMaterial {
    /// Which BIP-352 account this material was derived for.
    pub fn account(&self) -> SilentPaymentAccount {
        self.account
    }

    /// `b_scan` — the scan secret, for the ECDH shared secret and label tweaks.
    pub fn scan_seckey(&self) -> SecretKey {
        self.scan_seckey
    }

    /// `B_spend = b_spend·G` — the public base point outputs are tweaked from.
    pub fn spend_pubkey(&self) -> PublicKey {
        self.spend_pubkey
    }

    /// Scan public key.
    pub fn scan_pubkey<C: elements_miniscript::elements::bitcoin::secp256k1::Signing>(
        &self,
        secp: &elements_miniscript::elements::bitcoin::secp256k1::Secp256k1<C>,
    ) -> PublicKey {
        self.scan_seckey.public_key(secp)
    }
    /// Assemble scan material for `account`.
    pub fn new(
        account: SilentPaymentAccount,
        scan_seckey: SecretKey,
        spend_pubkey: PublicKey,
    ) -> Self {
        SilentPaymentScanMaterial {
            account,
            scan_seckey,
            spend_pubkey,
        }
    }

    /// PSET metadata for an output's spend tweak.
    pub fn input_meta(&self, spend_tweak: Scalar) -> SilentPaymentInputMeta {
        SilentPaymentInputMeta {
            account: self.account,
            spend_tweak,
            expected_spend_pubkey: self.spend_pubkey,
        }
    }
}

/// Silent-payment PSET metadata.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SilentPaymentInputMeta {
    account: SilentPaymentAccount,
    spend_tweak: Scalar,
    expected_spend_pubkey: PublicKey,
}

/// Silent-payment operations offered by a signer.
pub trait SilentPaymentSigner: Signer {
    /// Export scan material for `account`.
    fn silent_payment_scan_material(
        &self,
        account: SilentPaymentAccount,
    ) -> Result<SilentPaymentScanMaterial, Self::Error>;
}

/// Errors reading silent-payment PSET metadata.
#[derive(thiserror::Error, Debug, Clone, Copy, PartialEq, Eq)]
pub enum SilentPaymentPsetMetaError {
    /// The proprietary key/value for silent payments was not present on this input.
    #[error("input carries no silent payment metadata")]
    Missing,

    /// The value was present but not the expected byte layout.
    #[error("silent payment metadata is malformed")]
    Malformed,

    /// The blob parsed, but named account coordinates that cannot be derived.
    #[error("silent payment metadata names an underivable account: {0}")]
    Account(#[from] SilentPaymentAccountError),
}

impl SilentPaymentInputMeta {
    /// Which account's `b_spend` this input's tweak is relative to.
    pub fn account(&self) -> SilentPaymentAccount {
        self.account
    }

    /// The scalar that turns the account's `b_spend` into this output's spend key.
    pub fn spend_tweak(&self) -> Scalar {
        self.spend_tweak
    }

    /// The `B_spend` the wallet says it derived this tweak from.
    pub fn expected_spend_pubkey(&self) -> PublicKey {
        self.expected_spend_pubkey
    }

    /// Proprietary-key prefix for silent-payment metadata.
    const PROPRIETARY_PREFIX: &'static [u8] = b"lwk_sp";

    /// Proprietary-key subtype for this input metadata blob.
    const SUBTYPE: u8 = 0x01;

    /// Encodes `coin_type || account || spend_tweak || expected_spend_pubkey`.
    fn to_bytes(self) -> Vec<u8> {
        let mut out = Vec::with_capacity(4 + 4 + 32 + 33);
        out.extend_from_slice(&self.account.coin_type.to_le_bytes());
        out.extend_from_slice(&self.account.account.to_le_bytes());
        out.extend_from_slice(&self.spend_tweak.to_be_bytes());
        out.extend_from_slice(&self.expected_spend_pubkey.serialize());
        out
    }

    fn from_bytes(bytes: &[u8]) -> Result<Self, SilentPaymentPsetMetaError> {
        if bytes.len() != 4 + 4 + 32 + 33 {
            return Err(SilentPaymentPsetMetaError::Malformed);
        }
        let coin_type = u32::from_le_bytes(bytes[0..4].try_into().expect("checked len"));
        let account = u32::from_le_bytes(bytes[4..8].try_into().expect("checked len"));
        let spend_tweak = Scalar::from_be_bytes(bytes[8..40].try_into().expect("checked len"))
            .map_err(|_| SilentPaymentPsetMetaError::Malformed)?;
        let expected_spend_pubkey = PublicKey::from_slice(&bytes[40..73])
            .map_err(|_| SilentPaymentPsetMetaError::Malformed)?;
        Ok(SilentPaymentInputMeta {
            account: SilentPaymentAccount::from_raw(coin_type, account)?,
            spend_tweak,
            expected_spend_pubkey,
        })
    }

    /// Builds the proprietary key without using the reserved `pset` prefix.
    fn proprietary_key() -> ProprietaryKey {
        ProprietaryKey {
            prefix: Self::PROPRIETARY_PREFIX.to_vec(),
            subtype: Self::SUBTYPE,
            key: vec![],
        }
    }

    /// Attach this metadata to a PSET input.
    ///
    /// Overwrites any silent-payment metadata already present on the input.
    pub fn attach(self, input: &mut PsetInput) {
        input
            .proprietary
            .insert(Self::proprietary_key(), self.to_bytes());
    }

    /// Read silent-payment metadata back out of a PSET input, if present.
    pub fn read(input: &PsetInput) -> Result<Self, SilentPaymentPsetMetaError> {
        let bytes = input
            .proprietary
            .get(&Self::proprietary_key())
            .ok_or(SilentPaymentPsetMetaError::Missing)?;
        Self::from_bytes(bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use elements_miniscript::elements::pset::PartiallySignedTransaction;
    use std::str::FromStr;

    fn sk(byte: u8) -> SecretKey {
        SecretKey::from_slice(&[byte; 32]).unwrap()
    }

    #[test]
    fn account_paths_follow_elip_convention() {
        let account = SilentPaymentAccount::liquid_mainnet(0);
        assert_eq!(
            account.scan_path(),
            DerivationPath::from_str("m/352'/1776'/0'/1'/0").unwrap()
        );
        assert_eq!(
            account.spend_path(),
            DerivationPath::from_str("m/352'/1776'/0'/0'/0").unwrap()
        );

        let testnet = SilentPaymentAccount::liquid_testnet(3);
        assert_eq!(
            testnet.scan_path(),
            DerivationPath::from_str("m/352'/1'/3'/1'/0").unwrap()
        );
        assert_eq!(
            testnet.spend_path(),
            DerivationPath::from_str("m/352'/1'/3'/0'/0").unwrap()
        );
    }

    #[test]
    fn un_hardenable_account_coordinates_are_refused() {
        for bad in [HARDENED_THRESHOLD, HARDENED_THRESHOLD + 1, u32::MAX] {
            assert_eq!(
                SilentPaymentAccount::from_raw(bad, 0),
                Err(SilentPaymentAccountError::CoinTypeNotHardenable(bad))
            );
            assert_eq!(
                SilentPaymentAccount::from_raw(1, bad),
                Err(SilentPaymentAccountError::AccountNotHardenable(bad))
            );
        }

        let edge = SilentPaymentAccount::from_raw(HARDENED_THRESHOLD - 1, HARDENED_THRESHOLD - 1)
            .expect("2^31 - 1 is a valid hardened index");
        let _ = edge.scan_path();
        let _ = edge.spend_path();
    }

    #[test]
    fn a_crafted_pset_naming_an_underivable_account_errors_instead_of_panicking() {
        let secp = elements_miniscript::elements::secp256k1_zkp::Secp256k1::new();
        let honest = SilentPaymentInputMeta {
            account: SilentPaymentAccount::liquid_testnet(0),
            spend_tweak: Scalar::from_be_bytes(sk(0x11).secret_bytes()).unwrap(),
            expected_spend_pubkey: sk(0x22).public_key(&secp),
        };
        let mut input = PsetInput::default();
        honest.attach(&mut input);

        let key = SilentPaymentInputMeta::proprietary_key();
        for (offset, expected) in [
            (
                0,
                SilentPaymentAccountError::CoinTypeNotHardenable(u32::MAX),
            ),
            (4, SilentPaymentAccountError::AccountNotHardenable(u32::MAX)),
        ] {
            let mut bytes = honest.to_bytes();
            bytes[offset..offset + 4].copy_from_slice(&u32::MAX.to_le_bytes());
            input.proprietary.insert(key.clone(), bytes);

            assert_eq!(
                SilentPaymentInputMeta::read(&input),
                Err(SilentPaymentPsetMetaError::Account(expected)),
                "crafted coordinates at offset {offset} must be rejected, not derived"
            );
        }
    }

    #[test]
    fn input_meta_roundtrips_through_a_pset_input() {
        let secp = elements_miniscript::elements::secp256k1_zkp::Secp256k1::new();
        let meta = SilentPaymentInputMeta {
            account: SilentPaymentAccount::liquid_mainnet(1),
            spend_tweak: Scalar::from_be_bytes(sk(0x42).secret_bytes()).unwrap(),
            expected_spend_pubkey: sk(0x24).public_key(&secp),
        };

        let mut input = PsetInput::default();
        assert_eq!(
            SilentPaymentInputMeta::read(&input),
            Err(SilentPaymentPsetMetaError::Missing)
        );

        meta.attach(&mut input);
        assert_eq!(SilentPaymentInputMeta::read(&input), Ok(meta));
    }

    #[test]
    fn metadata_survives_a_pset_serialization_roundtrip() {
        use elements_miniscript::elements::encode::{deserialize, serialize};
        use elements_miniscript::elements::OutPoint;

        let secp = elements_miniscript::elements::secp256k1_zkp::Secp256k1::new();
        let meta = SilentPaymentInputMeta {
            account: SilentPaymentAccount::liquid_testnet(2),
            spend_tweak: Scalar::from_be_bytes(sk(0x42).secret_bytes()).unwrap(),
            expected_spend_pubkey: sk(0x24).public_key(&secp),
        };

        let mut pset = PartiallySignedTransaction::new_v2();
        let mut input = PsetInput::from_prevout(OutPoint::default());
        meta.attach(&mut input);
        pset.add_input(input);

        let bytes = serialize(&pset);
        let decoded: PartiallySignedTransaction =
            deserialize(&bytes).expect("a PSET carrying SP metadata must deserialize");

        assert_eq!(
            SilentPaymentInputMeta::read(&decoded.inputs()[0]),
            Ok(meta),
            "metadata must survive the round trip byte-for-byte"
        );
    }

    #[test]
    fn proprietary_key_uses_our_own_namespace() {
        let key = SilentPaymentInputMeta::proprietary_key();
        assert_eq!(key.prefix, SilentPaymentInputMeta::PROPRIETARY_PREFIX);
        assert!(
            !key.is_pset_key(),
            "must not claim the reserved `pset` namespace"
        );
    }

    #[test]
    fn malformed_metadata_is_reported_not_panicked() {
        let mut input = PsetInput::default();
        input
            .proprietary
            .insert(SilentPaymentInputMeta::proprietary_key(), vec![0u8; 3]);
        assert_eq!(
            SilentPaymentInputMeta::read(&input),
            Err(SilentPaymentPsetMetaError::Malformed)
        );
    }
}
