use core::convert::From;
use core::iter::IntoIterator;
use core::str::FromStr;

use elements_miniscript::elements::bitcoin::{
    bip32::{DerivationPath, Error, Fingerprint, KeySource, Xpub},
    consensus::encode::{self, VarInt},
    hashes::{sha256, Hash, HashEngine},
};

use crate::merkle::MerkleTree;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Version {
    V1 = 1,
    V2 = 2,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum AddressType {
    /// Legacy address type. P2PKH for single sig, P2SH for scripts.
    Legacy,
    /// Native segwit v0 address type. P2WPKH for single sig, P2WPSH for scripts.
    NativeSegwit,
    /// Nested segwit v0 address type. P2SH-P2WPKH for single sig, P2SH-P2WPSH for scripts.
    NestedSegwit,
    /// Segwit v1 Taproot address type. P2TR always.
    Taproot,
}

/// Represents a wallet stored with a wallet policy.
#[derive(Clone)]
pub struct WalletPolicy {
    /// wallet name (ASCII string, max 64 bytes)
    pub name: String,
    /// wallet version
    pub version: Version,
    /// descriptor with keys aliased by '@i',
    /// i the index of the key in the keys array.
    pub descriptor_template: String,
    /// Keys are the extended pubkeys used in the descriptor.
    pub keys: Vec<WalletPubKey>,
    /// Threshold of the multisig policy,
    /// None if the policy is not a multisig.
    pub threshold: Option<usize>,
}

impl WalletPolicy {
    pub fn new(
        name: String,
        version: Version,
        descriptor_template: String,
        keys: impl IntoIterator<Item = impl Into<WalletPubKey>>,
    ) -> Self {
        Self {
            name,
            version,
            descriptor_template,
            keys: keys.into_iter().map(|k| k.into()).collect(),
            threshold: None,
        }
    }

    pub fn new_multisig<T: Into<WalletPubKey>>(
        name: String,
        version: Version,
        address_type: AddressType,
        threshold: usize,
        keys: impl IntoIterator<Item = T>,
        sorted: bool,
        // TODO: use better type
        descriptor_blinding_key: Option<String>,
    ) -> Result<Self, WalletError> {
        let keys: Vec<WalletPubKey> = keys.into_iter().map(|k| k.into()).collect();
        if threshold < 1 || threshold > keys.len() {
            return Err(WalletError::InvalidThreshold);
        }

        let key_placeholder_suffix = if version == Version::V2 { "/**" } else { "" };
        let multisig_op = if sorted { "sortedmulti" } else { "multi" };
        let keys_str = keys
            .iter()
            .enumerate()
            .map(|(i, _)| format!("@{i}{key_placeholder_suffix}"))
            .collect::<Vec<String>>()
            .join(",");

        let mut descriptor_template = match address_type {
            AddressType::Legacy => format!("sh({multisig_op}({threshold},{keys_str}))"),
            AddressType::NativeSegwit => {
                format!("wsh({multisig_op}({threshold},{keys_str}))")
            }
            AddressType::NestedSegwit => {
                format!("sh(wsh({multisig_op}({threshold},{keys_str})))")
            }
            _ => return Err(WalletError::UnsupportedAddressType),
        };

        if let Some(descriptor_blinding_key) = descriptor_blinding_key {
            descriptor_template = format!("ct({descriptor_blinding_key},{descriptor_template})");
        }

        Ok(Self {
            name,
            version,
            descriptor_template,
            keys,
            threshold: Some(threshold),
        })
    }

    pub fn serialize(&self) -> Vec<u8> {
        let mut res: Vec<u8> = (self.version as u8).to_be_bytes().to_vec();
        res.extend_from_slice(&(self.name.len() as u8).to_be_bytes());
        res.extend_from_slice(self.name.as_bytes());
        res.extend(encode::serialize(&VarInt(
            self.descriptor_template.len() as u64
        )));

        if self.version == Version::V2 {
            let mut engine = sha256::Hash::engine();
            engine.input(self.descriptor_template.as_bytes());
            let hash = sha256::Hash::from_engine(engine).to_byte_array();
            res.extend_from_slice(&hash);
        } else {
            res.extend_from_slice(self.descriptor_template.as_bytes());
        }

        res.extend(encode::serialize(&VarInt(self.keys.len() as u64)));

        res.extend_from_slice(
            MerkleTree::new(
                self.keys
                    .iter()
                    .map(|key| {
                        let mut preimage = vec![0x00];
                        preimage.extend_from_slice(key.to_string().as_bytes());
                        let mut engine = sha256::Hash::engine();
                        engine.input(&preimage);
                        sha256::Hash::from_engine(engine).to_byte_array()
                    })
                    .collect(),
            )
            .root_hash(),
        );

        res
    }

    pub fn get_descriptor(&self, change: bool) -> Result<String, WalletError> {
        let mut desc = self.descriptor_template.clone();

        for (i, key) in self.keys.iter().enumerate().rev() {
            desc = desc.replace(&format!("@{i}"), &key.to_string());
        }

        desc = desc.replace("/**", &format!("/{}/{}", if change { 1 } else { 0 }, "*"));

        // For every "/<M;N>" expression, replace with M if not change, or with N if change
        while let Some(start) = desc.find("/<") {
            if let Some(end) = desc.find('>') {
                let nums: Vec<&str> = desc[start + 2..end].split(';').collect();
                if nums.len() == 2 {
                    let replacement = if change { nums[1] } else { nums[0] };
                    desc = format!("{}{}{}", &desc[..start + 1], replacement, &desc[end + 1..]);
                } else {
                    return Err(WalletError::InvalidPolicy);
                }
            }
        }

        Ok(desc)
    }

    pub fn id(&self) -> [u8; 32] {
        let mut engine = sha256::Hash::engine();
        engine.input(&self.serialize());
        sha256::Hash::from_engine(engine).to_byte_array()
    }
}

#[derive(Debug)]
pub enum WalletError {
    InvalidThreshold,
    UnsupportedAddressType,
    InvalidPolicy,
}

#[derive(PartialEq, Eq, Clone)]
pub struct WalletPubKey {
    pub inner: Xpub,
    pub source: Option<KeySource>,

    /// Used by Version V1
    /// either /** or /<NUM;NUM>/*
    pub multipath: Option<String>,
}

impl From<Xpub> for WalletPubKey {
    fn from(inner: Xpub) -> Self {
        Self {
            inner,
            source: None,
            multipath: None,
        }
    }
}

impl From<(KeySource, Xpub)> for WalletPubKey {
    fn from(source_xpub: (KeySource, Xpub)) -> Self {
        Self {
            inner: source_xpub.1,
            source: Some(source_xpub.0),
            multipath: None,
        }
    }
}

impl From<(KeySource, Xpub, String)> for WalletPubKey {
    fn from(source_xpub: (KeySource, Xpub, String)) -> Self {
        Self {
            inner: source_xpub.1,
            source: Some(source_xpub.0),
            multipath: Some(source_xpub.2),
        }
    }
}

impl FromStr for WalletPubKey {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if let Ok(key) = Xpub::from_str(s) {
            Ok(WalletPubKey {
                inner: key,
                source: None,
                multipath: None,
            })
        } else {
            let (keysource_str, xpub_str) = s
                .strip_prefix('[')
                .and_then(|s| s.rsplit_once(']'))
                .ok_or(Error::InvalidDerivationPathFormat)?;
            let (f_str, path_str) = keysource_str.split_once('/').unwrap_or((keysource_str, ""));
            let fingerprint =
                Fingerprint::from_str(f_str).map_err(|_| Error::InvalidDerivationPathFormat)?;
            let derivation_path = if path_str.is_empty() {
                DerivationPath::master()
            } else {
                DerivationPath::from_str(&format!("m/{path_str}"))?
            };
            let (xpub_str, multipath) = if let Some((xpub, multipath)) = xpub_str.rsplit_once('/') {
                (xpub, Some(format!("/{multipath}")))
            } else {
                (xpub_str, None)
            };
            Ok(WalletPubKey {
                inner: Xpub::from_str(xpub_str)?,
                source: Some((fingerprint, derivation_path)),
                multipath,
            })
        }
    }
}

impl core::fmt::Display for WalletPubKey {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        match &self.source {
            None => write!(
                f,
                "{}{}",
                self.inner,
                self.multipath.as_ref().unwrap_or(&"".to_string())
            ),
            Some((fg, path)) => {
                if path.is_master() {
                    write!(
                        f,
                        "[{}]{}{}",
                        fg,
                        self.inner,
                        self.multipath.as_ref().unwrap_or(&"".to_string())
                    )
                } else {
                    write!(
                        f,
                        "[{}/{}]{}{}",
                        fg,
                        path,
                        self.inner,
                        self.multipath.as_ref().unwrap_or(&"".to_string())
                    )
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::str::FromStr;
    use elements_miniscript::elements::bitcoin::hashes::hex::FromHex;

    const MASTER_KEY_EXAMPLE: &str = "[5c9e228d]tpubDEGquuorgFNb8bjh5kNZQMPtABJzoWwNm78FUmeoPkfRtoPF7JLrtoZeT3J3ybq1HmC3Rn1Q8wFQ8J5usanzups5rj7PJoQLNyvq8QbJruW/**";
    const KEY_EXAMPLE: &str = "[5c9e228d/48'/1'/0'/0']tpubDEGquuorgFNb8bjh5kNZQMPtABJzoWwNm78FUmeoPkfRtoPF7JLrtoZeT3J3ybq1HmC3Rn1Q8wFQ8J5usanzups5rj7PJoQLNyvq8QbJruW/**";

    #[test]
    fn test_master_walletpubkey_fromstr() {
        let key = WalletPubKey::from_str(MASTER_KEY_EXAMPLE).unwrap();
        assert_eq!(
            key.source.as_ref().unwrap().0,
            Fingerprint::from_str("5c9e228d").unwrap()
        );
        assert_eq!(key.source.as_ref().unwrap().1, DerivationPath::master());
        assert_eq!(key.inner, Xpub::from_str("tpubDEGquuorgFNb8bjh5kNZQMPtABJzoWwNm78FUmeoPkfRtoPF7JLrtoZeT3J3ybq1HmC3Rn1Q8wFQ8J5usanzups5rj7PJoQLNyvq8QbJruW").unwrap());
        assert_eq!(key.multipath, Some("/**".to_string()));
    }

    #[test]
    fn test_walletpubkey_fromstr() {
        let key = WalletPubKey::from_str(KEY_EXAMPLE).unwrap();
        assert_eq!(
            key.source.as_ref().unwrap().0,
            Fingerprint::from_str("5c9e228d").unwrap()
        );
        assert_eq!(
            key.source.as_ref().unwrap().1,
            DerivationPath::from_str("m/48'/1'/0'/0'").unwrap()
        );
        assert_eq!(key.inner, Xpub::from_str("tpubDEGquuorgFNb8bjh5kNZQMPtABJzoWwNm78FUmeoPkfRtoPF7JLrtoZeT3J3ybq1HmC3Rn1Q8wFQ8J5usanzups5rj7PJoQLNyvq8QbJruW").unwrap());
        assert_eq!(key.multipath, Some("/**".to_string()));
    }

    #[test]
    fn test_walletpubkey_tostr() {
        let key = WalletPubKey::from_str(KEY_EXAMPLE).unwrap();
        assert_eq!(key.to_string(), format!("{KEY_EXAMPLE}"));
    }

    #[test]
    fn test_wallet_serialize_v2() {
        let wallet = WalletPolicy::new(
            "Cold storage".to_string(),
            Version::V2,
            "wsh(sortedmulti(2,@0/**,@1/**))".to_string(),
            vec![
               WalletPubKey::from_str("[76223a6e/48'/1'/0'/2']tpubDE7NQymr4AFtewpAsWtnreyq9ghkzQBXpCZjWLFVRAvnbf7vya2eMTvT2fPapNqL8SuVvLQdbUbMfWLVDCZKnsEBqp6UK93QEzL8Ck23AwF").unwrap(),
               WalletPubKey::from_str("[f5acc2fd/48'/1'/0'/2']tpubDFAqEGNyad35aBCKUAXbQGDjdVhNueno5ZZVEn3sQbW5ci457gLR7HyTmHBg93oourBssgUxuWz1jX5uhc1qaqFo9VsybY1J5FuedLfm4dK").unwrap(),
            ],
        );
        assert_eq!(wallet.serialize().as_slice(), Vec::<u8>::from_hex("020c436f6c642073746f726167651fb56c3d5542fa09b3956834a9ff6a1df5c36a38e5b02c63c54b41a9a04403b82602516d2c50a89476ecffeec658057f0110674bbfafc18797dc480c7ed53802f3fb").unwrap());
    }

    #[test]
    fn test_get_descriptor() {
        let wallet = WalletPolicy::new(
            "Cold storage".to_string(),
            Version::V2,
            "wsh(sortedmulti(2,@0/**,@1/<12;3>/*))".to_string(),
            vec![
               WalletPubKey::from_str("[76223a6e/48'/1'/0'/2']tpubDE7NQymr4AFtewpAsWtnreyq9ghkzQBXpCZjWLFVRAvnbf7vya2eMTvT2fPapNqL8SuVvLQdbUbMfWLVDCZKnsEBqp6UK93QEzL8Ck23AwF").unwrap(),
               WalletPubKey::from_str("[f5acc2fd/48'/1'/0'/2']tpubDFAqEGNyad35aBCKUAXbQGDjdVhNueno5ZZVEn3sQbW5ci457gLR7HyTmHBg93oourBssgUxuWz1jX5uhc1qaqFo9VsybY1J5FuedLfm4dK").unwrap(),
            ],
        );

        assert_eq!(wallet.get_descriptor(false).unwrap(), "wsh(sortedmulti(2,[76223a6e/48'/1'/0'/2']tpubDE7NQymr4AFtewpAsWtnreyq9ghkzQBXpCZjWLFVRAvnbf7vya2eMTvT2fPapNqL8SuVvLQdbUbMfWLVDCZKnsEBqp6UK93QEzL8Ck23AwF/0/*,[f5acc2fd/48'/1'/0'/2']tpubDFAqEGNyad35aBCKUAXbQGDjdVhNueno5ZZVEn3sQbW5ci457gLR7HyTmHBg93oourBssgUxuWz1jX5uhc1qaqFo9VsybY1J5FuedLfm4dK/12/*))");
        assert_eq!(wallet.get_descriptor(true).unwrap(), "wsh(sortedmulti(2,[76223a6e/48'/1'/0'/2']tpubDE7NQymr4AFtewpAsWtnreyq9ghkzQBXpCZjWLFVRAvnbf7vya2eMTvT2fPapNqL8SuVvLQdbUbMfWLVDCZKnsEBqp6UK93QEzL8Ck23AwF/1/*,[f5acc2fd/48'/1'/0'/2']tpubDFAqEGNyad35aBCKUAXbQGDjdVhNueno5ZZVEn3sQbW5ci457gLR7HyTmHBg93oourBssgUxuWz1jX5uhc1qaqFo9VsybY1J5FuedLfm4dK/3/*))");

        let wallet = WalletPolicy::new(
            "Cold storage".to_string(),
            Version::V2,
            "wsh(or_d(pk(@0/<0;1>/*),and_v(v:pkh(@1/<0;1>/*),older(65535))))".to_string(),
            vec![
               WalletPubKey::from_str("[ffd63c8d/48'/1'/0'/2']tpubDExA3EC3iAsPxPhFn4j6gMiVup6V2eH3qKyk69RcTc9TTNRfFYVPad8bJD5FCHVQxyBT4izKsvr7Btd2R4xmQ1hZkvsqGBaeE82J71uTK4N").unwrap(),
               WalletPubKey::from_str("[053f423f/48'/1'/0'/2']tpubDEGZMZiz8Vnp7N7cTM9Cty897GJpQ8jqmw2yyDKMPfbMzqPtRbo8wViKtkx6zfrzY6jW5NPNULeN9j7oYCqvrFxCkhSdJs7QxwZ3qQ1PXSp").unwrap(),
            ],
        );
        assert_eq!(wallet.get_descriptor(false).unwrap(), "wsh(or_d(pk([ffd63c8d/48'/1'/0'/2']tpubDExA3EC3iAsPxPhFn4j6gMiVup6V2eH3qKyk69RcTc9TTNRfFYVPad8bJD5FCHVQxyBT4izKsvr7Btd2R4xmQ1hZkvsqGBaeE82J71uTK4N/0/*),and_v(v:pkh([053f423f/48'/1'/0'/2']tpubDEGZMZiz8Vnp7N7cTM9Cty897GJpQ8jqmw2yyDKMPfbMzqPtRbo8wViKtkx6zfrzY6jW5NPNULeN9j7oYCqvrFxCkhSdJs7QxwZ3qQ1PXSp/0/*),older(65535))))");

        assert_eq!(wallet.get_descriptor(true).unwrap(), "wsh(or_d(pk([ffd63c8d/48'/1'/0'/2']tpubDExA3EC3iAsPxPhFn4j6gMiVup6V2eH3qKyk69RcTc9TTNRfFYVPad8bJD5FCHVQxyBT4izKsvr7Btd2R4xmQ1hZkvsqGBaeE82J71uTK4N/1/*),and_v(v:pkh([053f423f/48'/1'/0'/2']tpubDEGZMZiz8Vnp7N7cTM9Cty897GJpQ8jqmw2yyDKMPfbMzqPtRbo8wViKtkx6zfrzY6jW5NPNULeN9j7oYCqvrFxCkhSdJs7QxwZ3qQ1PXSp/1/*),older(65535))))");

        let wallet = WalletPolicy::new(
            "Cold storage".to_string(),
            Version::V2,
            "wsh(or_d(pk(@0/<0;1>/*),and_v(v:pkh(@1/**),older(65535))))".to_string(),
            vec![
               WalletPubKey::from_str("[ffd63c8d/48'/1'/0'/2']tpubDExA3EC3iAsPxPhFn4j6gMiVup6V2eH3qKyk69RcTc9TTNRfFYVPad8bJD5FCHVQxyBT4izKsvr7Btd2R4xmQ1hZkvsqGBaeE82J71uTK4N").unwrap(),
               WalletPubKey::from_str("[053f423f/48'/1'/0'/2']tpubDEGZMZiz8Vnp7N7cTM9Cty897GJpQ8jqmw2yyDKMPfbMzqPtRbo8wViKtkx6zfrzY6jW5NPNULeN9j7oYCqvrFxCkhSdJs7QxwZ3qQ1PXSp").unwrap(),
            ],
        );
        assert_eq!(wallet.get_descriptor(false).unwrap(), "wsh(or_d(pk([ffd63c8d/48'/1'/0'/2']tpubDExA3EC3iAsPxPhFn4j6gMiVup6V2eH3qKyk69RcTc9TTNRfFYVPad8bJD5FCHVQxyBT4izKsvr7Btd2R4xmQ1hZkvsqGBaeE82J71uTK4N/0/*),and_v(v:pkh([053f423f/48'/1'/0'/2']tpubDEGZMZiz8Vnp7N7cTM9Cty897GJpQ8jqmw2yyDKMPfbMzqPtRbo8wViKtkx6zfrzY6jW5NPNULeN9j7oYCqvrFxCkhSdJs7QxwZ3qQ1PXSp/0/*),older(65535))))");

        assert_eq!(wallet.get_descriptor(true).unwrap(), "wsh(or_d(pk([ffd63c8d/48'/1'/0'/2']tpubDExA3EC3iAsPxPhFn4j6gMiVup6V2eH3qKyk69RcTc9TTNRfFYVPad8bJD5FCHVQxyBT4izKsvr7Btd2R4xmQ1hZkvsqGBaeE82J71uTK4N/1/*),and_v(v:pkh([053f423f/48'/1'/0'/2']tpubDEGZMZiz8Vnp7N7cTM9Cty897GJpQ8jqmw2yyDKMPfbMzqPtRbo8wViKtkx6zfrzY6jW5NPNULeN9j7oYCqvrFxCkhSdJs7QxwZ3qQ1PXSp/1/*),older(65535))))");

        let wallet = WalletPolicy::new(
            "Cold storage".to_string(),
            Version::V2,
            "wsh(or_d(pk(@0/**),and_v(v:pkh(@1/**),older(65535))))".to_string(),
            vec![
               WalletPubKey::from_str("[ffd63c8d/48'/1'/0'/2']tpubDExA3EC3iAsPxPhFn4j6gMiVup6V2eH3qKyk69RcTc9TTNRfFYVPad8bJD5FCHVQxyBT4izKsvr7Btd2R4xmQ1hZkvsqGBaeE82J71uTK4N").unwrap(),
               WalletPubKey::from_str("[053f423f/48'/1'/0'/2']tpubDEGZMZiz8Vnp7N7cTM9Cty897GJpQ8jqmw2yyDKMPfbMzqPtRbo8wViKtkx6zfrzY6jW5NPNULeN9j7oYCqvrFxCkhSdJs7QxwZ3qQ1PXSp").unwrap(),
            ],
        );
        assert_eq!(wallet.get_descriptor(false).unwrap(), "wsh(or_d(pk([ffd63c8d/48'/1'/0'/2']tpubDExA3EC3iAsPxPhFn4j6gMiVup6V2eH3qKyk69RcTc9TTNRfFYVPad8bJD5FCHVQxyBT4izKsvr7Btd2R4xmQ1hZkvsqGBaeE82J71uTK4N/0/*),and_v(v:pkh([053f423f/48'/1'/0'/2']tpubDEGZMZiz8Vnp7N7cTM9Cty897GJpQ8jqmw2yyDKMPfbMzqPtRbo8wViKtkx6zfrzY6jW5NPNULeN9j7oYCqvrFxCkhSdJs7QxwZ3qQ1PXSp/0/*),older(65535))))");

        assert_eq!(wallet.get_descriptor(true).unwrap(), "wsh(or_d(pk([ffd63c8d/48'/1'/0'/2']tpubDExA3EC3iAsPxPhFn4j6gMiVup6V2eH3qKyk69RcTc9TTNRfFYVPad8bJD5FCHVQxyBT4izKsvr7Btd2R4xmQ1hZkvsqGBaeE82J71uTK4N/1/*),and_v(v:pkh([053f423f/48'/1'/0'/2']tpubDEGZMZiz8Vnp7N7cTM9Cty897GJpQ8jqmw2yyDKMPfbMzqPtRbo8wViKtkx6zfrzY6jW5NPNULeN9j7oYCqvrFxCkhSdJs7QxwZ3qQ1PXSp/1/*),older(65535))))");
    }
}
