use std::{fmt::Display, str::FromStr};

#[allow(deprecated)]
use aes_gcm_siv::aead::generic_array::GenericArray;
use aes_gcm_siv::Aes256GcmSiv;
use aes_gcm_siv::KeyInit;
use elements::bitcoin::secp256k1::SecretKey;
use elements::bitcoin::{bip32::ChildNumber, WitnessVersion};
use elements::hashes::{sha256t_hash_newtype, Hash};
use elements::{bitcoin, Address, AddressParams, Script};
use elements_miniscript::BtcDescriptor;
use elements_miniscript::DefiniteDescriptorKey;
use elements_miniscript::{
    confidential::Key,
    descriptor::{DescriptorSecretKey, Wildcard},
    ConfidentialDescriptor, Descriptor, DescriptorPublicKey, ForEachKey,
};
use serde::{Deserialize, Serialize};

use crate::{Error, EC};

sha256t_hash_newtype! {
    /// The tag of the hash
    pub struct EncryptionKeyTag = hash_str("LWK-FS-Encryption-Key/1.0");

    /// A tagged hash to generate the key for encryption in the encrypted file system persister
    #[hash_newtype(forward)]
    pub struct EncryptionKeyHash(_);
}

#[derive(Debug, Clone)]
#[allow(clippy::large_enum_variant)]
enum DescOrSpks {
    Desc(ConfidentialDescriptor<DescriptorPublicKey>),
    Spks(Vec<Spk>),
}

#[derive(Debug, Clone)]
struct Spk {
    blinding_key: Option<SecretKey>,
    script_pubkey: Script,
}

impl Display for Spk {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.blinding_key {
            Some(key) => write!(f, "{}:{:x}", key.display_secret(), self.script_pubkey),
            None => write!(f, ":{:x}", self.script_pubkey),
        }
    }
}

impl FromStr for Spk {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (key_hex, spk_hex) = s.split_once(':').ok_or(Error::InvalidSpkFormat)?;
        let blinding_key = if key_hex.is_empty() {
            None
        } else {
            Some(SecretKey::from_str(key_hex)?)
        };
        let script_pubkey = Script::from_str(spk_hex)?;
        Ok(Spk {
            blinding_key,
            script_pubkey,
        })
    }
}

#[derive(Debug, Clone)]
/// A wrapper that contains only the subset of CT descriptors handled by wollet
pub struct WolletDescriptor {
    inner: DescOrSpks,
    #[cfg(feature = "amp0")]
    is_amp0: bool,
}

impl Display for WolletDescriptor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.inner {
            DescOrSpks::Desc(d) => Display::fmt(&d, f),
            DescOrSpks::Spks(spks) => {
                let parts: Vec<String> = spks.iter().map(|s| s.to_string()).collect();
                write!(f, "{}", parts.join(","))
            }
        }
    }
}

impl serde::Serialize for WolletDescriptor {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> serde::Deserialize<'de> for WolletDescriptor {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        s.parse().map_err(serde::de::Error::custom)
    }
}

impl std::hash::Hash for WolletDescriptor {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.to_string().hash(state);
    }
}

impl PartialEq for WolletDescriptor {
    fn eq(&self, other: &Self) -> bool {
        self.to_string() == other.to_string()
    }
}

impl Eq for WolletDescriptor {}

#[cfg(feature = "amp0")]
fn desc_is_amp0(desc: &ConfidentialDescriptor<DescriptorPublicKey>) -> bool {
    use crate::amp0;
    use elements_miniscript::descriptor::{ShInner, Wildcard, WshInner};
    use elements_miniscript::miniscript::decode::Terminal;

    // ct(slip77(...),elsh(wsh(multi(2,server_xpub,user_xpub))))
    if let Key::Slip77(_) = desc.key {
        if let Descriptor::Sh(sh) = &desc.descriptor {
            if let ShInner::Wsh(wsh) = sh.as_inner() {
                if let WshInner::Ms(ms) = wsh.as_inner() {
                    if let Terminal::Multi(2, pks) = &ms.node {
                        if let [DescriptorPublicKey::XPub(xkey), _] = &pks[..] {
                            // server xpub [fp/3/gaitpath/amp_subaccount]xpub/*
                            // user xpub   [fp/3'/amp_subaccount']xpub/* (not checked)
                            if xkey.wildcard == Wildcard::Unhardened
                                && xkey.derivation_path.is_empty()
                            {
                                if let Some((fingerprint, server_path)) = &xkey.origin {
                                    let fp = fingerprint.to_string();
                                    let cn = ChildNumber::Normal { index: 3 };
                                    if server_path.len() == 34
                                        && server_path[0] == cn
                                        && (fp == amp0::AMP0_FINGERPRINT_MAINNET
                                            || fp == amp0::AMP0_FINGERPRINT_TESTNET
                                            || fp == amp0::AMP0_FINGERPRINT_REGTEST)
                                    {
                                        return true;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    false
}

impl TryFrom<ConfidentialDescriptor<DescriptorPublicKey>> for WolletDescriptor {
    type Error = Error;

    fn try_from(desc: ConfidentialDescriptor<DescriptorPublicKey>) -> Result<Self, Self::Error> {
        if let Key::Bare(_) = &desc.key {
            return Err(Self::Error::BlindingBareUnsupported);
        }
        if let Key::View(DescriptorSecretKey::MultiXPrv(_)) = &desc.key {
            return Err(Self::Error::BlindingViewMultiUnsupported);
        }
        if let Key::View(DescriptorSecretKey::XPrv(k)) = &desc.key {
            if k.wildcard != Wildcard::None {
                return Err(Self::Error::BlindingViewWildcardUnsupported);
            }
        }

        if desc.descriptor.is_multipath() {
            let descriptors = desc.descriptor.clone().into_single_descriptors()?;

            if descriptors.len() > 2 {
                return Err(Self::Error::UnsupportedMultipathDescriptor);
            }

            for (i, desc) in descriptors.iter().enumerate() {
                let r = desc.for_each_key(|k| {
                    if let Some(path) = k.full_derivation_path() {
                        if let Some(val) = path.into_iter().last() {
                            return val == &ChildNumber::from(i as u32);
                        }
                    }
                    false
                });
                if !r {
                    return Err(Self::Error::UnsupportedMultipathDescriptor);
                }
            }
        }

        #[cfg(feature = "amp0")]
        let is_amp0 = desc_is_amp0(&desc);

        // Support legacy p2sh multisig
        if let elements_miniscript::descriptor::DescriptorType::Sh = desc.descriptor.desc_type() {
            if desc.descriptor.to_string().starts_with("elsh(multi(") {
                return Ok(WolletDescriptor {
                    inner: DescOrSpks::Desc(desc),
                    #[cfg(feature = "amp0")]
                    is_amp0,
                });
            }
        }

        match desc.descriptor.desc_type().segwit_version() {
            None => Err(Self::Error::UnsupportedDescriptorPreSegwit),
            Some(WitnessVersion::V0) => Ok(WolletDescriptor {
                inner: DescOrSpks::Desc(desc),
                #[cfg(feature = "amp0")]
                is_amp0,
            }),
            Some(WitnessVersion::V1) => Ok(WolletDescriptor {
                inner: DescOrSpks::Desc(desc),
                #[cfg(feature = "amp0")]
                is_amp0,
            }),
            Some(_) => Err(Self::Error::UnsupportedDescriptorSegwitUnknownVersion),
        }
    }
}

impl FromStr for WolletDescriptor {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match ConfidentialDescriptor::<DescriptorPublicKey>::from_str(s) {
            Ok(desc) => desc.try_into(),
            Err(first_err) => {
                // Try parsing as comma-separated "blinding_key_hex:spk_hex" pairs
                let spks: Result<Vec<Spk>, _> = s.split(',').map(Spk::from_str).collect();
                match spks {
                    Ok(spks) if !spks.is_empty() => Ok(WolletDescriptor {
                        inner: DescOrSpks::Spks(spks),
                        #[cfg(feature = "amp0")]
                        is_amp0: false,
                    }),
                    _ => Err(first_err.into()),
                }
            }
        }
    }
}

/// The chain can be either External or Internal.
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Hash, Clone, Copy, PartialOrd, Ord)]
pub enum Chain {
    /// External address, shown when asked for a payment.
    /// Wallet having a single descriptor are considered External
    External,

    /// Internal address, used for the change
    Internal,
}

impl TryFrom<&Descriptor<DescriptorPublicKey>> for Chain {
    type Error = ();

    fn try_from(value: &Descriptor<DescriptorPublicKey>) -> Result<Self, Self::Error> {
        let mut ext_int = None;
        // TODO: error if multiple keys have different derivation path

        let has_wildcard = value.has_wildcard();

        value.for_each_key(|k| {
            if let Some(path) = k.full_derivation_path() {
                if has_wildcard {
                    ext_int = path.into_iter().last().cloned();
                } else {
                    ext_int = path.into_iter().nth_back(1).cloned();
                }
            }
            false
        });
        match ext_int {
            None => Err(()),
            Some(ext_int) => Ok(ext_int.try_into()?),
        }
    }
}
impl TryFrom<ChildNumber> for Chain {
    type Error = ();

    fn try_from(value: ChildNumber) -> Result<Self, Self::Error> {
        match value {
            ChildNumber::Normal { index: 0 } => Ok(Chain::External),
            ChildNumber::Normal { index: 1 } => Ok(Chain::Internal),
            _ => Err(()),
        }
    }
}

impl TryFrom<&ConfidentialDescriptor<DescriptorPublicKey>> for Chain {
    type Error = ();

    fn try_from(value: &ConfidentialDescriptor<DescriptorPublicKey>) -> Result<Self, Self::Error> {
        (&value.descriptor).try_into()
    }
}

impl WolletDescriptor {
    /// Return a reference to the underlying descriptor.
    pub fn descriptor(&self) -> Result<&Descriptor<DescriptorPublicKey>, Error> {
        match &self.inner {
            DescOrSpks::Desc(d) => Ok(&d.descriptor),
            DescOrSpks::Spks(_) => Err(Error::UnsupportedWithoutDescriptor),
        }
    }

    /// Return a reference to the underlying CT descriptor.
    pub fn ct_descriptor(&self) -> Result<&ConfidentialDescriptor<DescriptorPublicKey>, Error> {
        match &self.inner {
            DescOrSpks::Desc(d) => Ok(d),
            DescOrSpks::Spks(_) => Err(Error::UnsupportedWithoutDescriptor),
        }
    }

    /// Return the descriptor URL encoded to be used as part of an URL
    pub fn url_encoded_descriptor(&self) -> Result<String, Error> {
        match &self.inner {
            DescOrSpks::Desc(d) => Ok(url_encode_descriptor(&d.to_string())),
            DescOrSpks::Spks(_) => Err(Error::UnsupportedWithoutDescriptor),
        }
    }

    /// Return the [ELIP152](https://github.com/ElementsProject/ELIPs/blob/main/elip-0152.mediawiki) deterministic wallet identifier.
    pub fn dwid(&self, network: lwk_common::Network) -> Result<String, Error> {
        let index = (1 << 31) - 1; // 2^31 - 1

        // Use the Elements network address parameters
        let params = network.address_params();
        let address = self.address(index, params)?;
        let address_str = address.to_string();
        let hash = elements::hashes::sha256::Hash::hash(address_str.as_bytes());
        let hash_hex = hash.to_string();

        // Take only the first half of the hash (32 hex characters = 16 bytes)
        let half_hash = &hash_hex[..32];

        // Format with hyphens every 4 characters
        let formatted = half_hash
            .chars()
            .collect::<Vec<_>>()
            .chunks(4)
            .map(|c| c.iter().collect::<String>())
            .collect::<Vec<_>>()
            .join("-");

        Ok(formatted)
    }

    /// Return wether the descriptor has a blinding key derived with [Elip151](https://github.com/ElementsProject/ELIPs/blob/main/elip-0151.mediawiki)
    pub fn is_elip151(&self) -> bool {
        match &self.inner {
            DescOrSpks::Desc(d) => {
                if let Ok(elip151_key) = Key::from_elip151(&d.descriptor) {
                    elip151_key == d.key
                } else {
                    false
                }
            }
            DescOrSpks::Spks(_) => false,
        }
    }

    /// Strip key origin information from the bitcoin descriptor and return it without checksum
    pub fn bitcoin_descriptor_without_key_origin(&self) -> Result<String, Error> {
        match &self.inner {
            DescOrSpks::Desc(d) => {
                let desc = d.descriptor.to_string();
                let mut result = String::with_capacity(desc.len());
                let mut skip = false;
                for c in desc.chars() {
                    if skip {
                        if c == ']' {
                            skip = false;
                        }
                    } else if c == '[' || c == '#' {
                        skip = true;
                    } else {
                        result.push(c)
                    }
                }
                Ok(result)
            }
            DescOrSpks::Spks(_) => Err(Error::UnsupportedWithoutDescriptor),
        }
    }

    /// return the single descriptor if not multipath, if multipath returns the internal or the
    /// external descriptor accordint to `int_or_ext`
    fn inner_descriptor_if_available(&self, ext_int: Chain) -> Result<WolletDescriptor, Error> {
        match &self.inner {
            DescOrSpks::Desc(d) => {
                let mut descriptors = d
                    .descriptor
                    .clone()
                    .into_single_descriptors()
                    .expect("already done in TryFrom");
                assert_ne!(descriptors.len(), 0);
                let descriptor = if descriptors.len() == 1 {
                    descriptors.pop().expect("inside len==1 branch")
                } else {
                    match ext_int {
                        Chain::External => descriptors.remove(0),
                        Chain::Internal => descriptors.remove(1),
                    }
                };
                let inner = ConfidentialDescriptor {
                    key: d.key.clone(),
                    descriptor,
                };
                Ok(WolletDescriptor {
                    inner: DescOrSpks::Desc(inner),
                    #[cfg(feature = "amp0")]
                    is_amp0: self.is_amp0,
                })
            }
            DescOrSpks::Spks(_) => Err(Error::UnsupportedWithoutDescriptor),
        }
    }

    fn is_multipath(&self) -> bool {
        match &self.inner {
            DescOrSpks::Desc(d) => d.descriptor.is_multipath(),
            DescOrSpks::Spks(_) => false,
        }
    }

    /// Derive the single path descriptors of this descriptor if it's multipath.
    /// If it's not multipath, return the descriptor itself as a single element.
    pub fn single_bitcoin_descriptors(&self) -> Result<Vec<String>, Error> {
        let d = self.inner_descriptor_if_available(Chain::External)?;
        let d = to_bitcoin_descriptor(&d.descriptor()?.to_string());
        let mut v = vec![d];
        if self.is_multipath() {
            let d = self.inner_descriptor_if_available(Chain::Internal)?;
            let d = to_bitcoin_descriptor(&d.descriptor()?.to_string());
            v.push(d);
        }
        Ok(v)
    }

    /// Derive a change address from this descriptor at the given `index`.
    pub fn change(&self, index: u32, params: &'static AddressParams) -> Result<Address, Error> {
        self.inner_address(index, params, Chain::Internal)
    }

    /// Get a cipher from the descriptor, used to encrypt and decrypt updates.
    ///
    /// The rationale to derive a key from the descriptor is to avoid storing a separate key that you need to backup for the wallet data.
    /// In the end the descriptor allows you to recover the same data directly from the blockchain, thus we don't need additional security.
    #[allow(deprecated)]
    pub fn cipher(&self) -> Aes256GcmSiv {
        let key_bytes = EncryptionKeyHash::hash(self.to_string().as_bytes()).to_byte_array();
        let key = GenericArray::from_slice(&key_bytes);
        Aes256GcmSiv::new(key)
    }

    /// Derive an address from this descriptor at the given `index`.
    pub fn address(&self, index: u32, params: &'static AddressParams) -> Result<Address, Error> {
        self.inner_address(index, params, Chain::External)
    }

    fn at_derivation_index(
        &self,
        index: u32,
    ) -> Result<ConfidentialDescriptor<DefiniteDescriptorKey>, Error> {
        match &self.inner {
            DescOrSpks::Desc(d) => Ok(d.at_derivation_index(index)?),
            DescOrSpks::Spks(_) => Err(Error::UnsupportedWithoutDescriptor),
        }
    }

    fn inner_address(
        &self,
        index: u32,
        params: &'static AddressParams,
        ext_int: Chain,
    ) -> Result<Address, Error> {
        #[cfg(feature = "amp0")]
        if self.is_amp0 {
            return Err(Error::Amp0AddressError);
        }
        match &self.inner {
            DescOrSpks::Spks(spks) => {
                let spk = spks.get(index as usize).ok_or(Error::IndexOutOfRange)?;
                let blinding_pk = spk.blinding_key.as_ref().map(|k| k.public_key(&EC));
                Address::from_script(&spk.script_pubkey, blinding_pk, params)
                    .ok_or(Error::UnsupportedWithoutDescriptor)
            }
            _ => Ok(self
                .inner_descriptor_if_available(ext_int)?
                .at_derivation_index(index)?
                .address(&EC, params)?),
        }
    }

    #[cfg(feature = "amp0")]
    pub(crate) fn amp0_address(
        &self,
        index: u32,
        params: &'static AddressParams,
    ) -> Result<Address, Error> {
        Ok(self.at_derivation_index(index)?.address(&EC, params)?)
    }

    /// Get a scriptpubkey
    pub fn script_pubkey(&self, ext_int: Chain, index: u32) -> Result<Script, Error> {
        match &self.inner {
            DescOrSpks::Spks(spks) => spks
                .get(index as usize)
                .map(|s| s.script_pubkey.clone())
                .ok_or(Error::IndexOutOfRange),
            _ => Ok(self
                .inner_descriptor_if_available(ext_int)?
                .at_derivation_index(index)?
                .descriptor
                .script_pubkey()),
        }
    }

    /// Get a definite descriptor
    pub fn definite_descriptor(
        &self,
        ext_int: Chain,
        index: u32,
    ) -> Result<Descriptor<DefiniteDescriptorKey>, Error> {
        let desc = self.inner_descriptor_if_available(ext_int)?;
        Ok(desc.descriptor()?.at_derivation_index(index)?)
    }

    /// Get a CT definite descriptor
    pub(crate) fn ct_definite_descriptor(
        &self,
        ext_int: Chain,
        index: u32,
    ) -> Result<ConfidentialDescriptor<DefiniteDescriptorKey>, Error> {
        self.inner_descriptor_if_available(ext_int)?
            .at_derivation_index(index)
    }

    /// Try also to parse it as a non-multipath descriptor specified on 2 lines,
    /// like the format exported by the Green Wallet
    pub fn from_str_relaxed(desc: &str) -> Result<WolletDescriptor, Error> {
        match WolletDescriptor::from_str(desc) {
            Ok(d) => Ok(d),
            Err(e) => parse_multiline(desc).ok_or(e),
        }
    }

    /// Returns true if all the xpubs in the descriptors are for mainnet
    pub fn is_mainnet(&self) -> bool {
        match &self.inner {
            DescOrSpks::Desc(d) => d.descriptor.for_each_key(|k| match k {
                DescriptorPublicKey::XPub(x) => {
                    x.xkey.network == elements::bitcoin::NetworkKind::Main
                }
                DescriptorPublicKey::MultiXPub(x) => {
                    x.xkey.network == elements::bitcoin::NetworkKind::Main
                }
                DescriptorPublicKey::Single(_) => true,
            }),
            _ => false,
        }
    }

    /// Return a bitcoin pegin address, btc sent to this address can be redeemed as lbtc
    pub fn pegin_address(
        &self,
        index: u32,
        network: bitcoin::Network,
        fed_desc: BtcDescriptor<bitcoin::PublicKey>,
    ) -> Result<bitcoin::Address, Error> {
        let our_desc = self
            .definite_descriptor(Chain::External, index)?
            .derived_descriptor(&EC)?;
        let pegin = elements_miniscript::descriptor::pegin::Pegin::new(fed_desc, our_desc);
        let pegin_script = pegin.bitcoin_witness_script(&EC)?;
        let pegin_address = bitcoin::Address::p2wsh(&pegin_script, network);
        Ok(pegin_address)
    }

    pub(crate) fn as_single_descriptors(
        &self,
    ) -> Result<Vec<ConfidentialDescriptor<DescriptorPublicKey>>, Error> {
        match &self.inner {
            DescOrSpks::Desc(d) => {
                let descriptors = d.descriptor.clone().into_single_descriptors()?;
                let mut result = Vec::with_capacity(descriptors.len());
                for descriptor in descriptors {
                    result.push(ConfidentialDescriptor {
                        key: d.key.clone(),
                        descriptor,
                    });
                }
                Ok(result)
            }
            DescOrSpks::Spks(_) => Err(Error::UnsupportedWithoutDescriptor),
        }
    }

    /// Return the blinding secret key for the given script pubkey.
    pub(crate) fn blinding_key_for_script(&self, script_pubkey: &Script) -> Option<SecretKey> {
        match &self.inner {
            DescOrSpks::Desc(d) => lwk_common::derive_blinding_key(d, script_pubkey),
            DescOrSpks::Spks(spks) => spks
                .iter()
                .find(|s| s.script_pubkey == *script_pubkey)
                .and_then(|s| s.blinding_key),
        }
    }

    /// Whether this descriptor has a wildcard. A descriptor without a wildcard is a single address descriptor.
    pub fn has_wildcard(&self) -> bool {
        match &self.inner {
            DescOrSpks::Desc(d) => d.descriptor.has_wildcard(),
            DescOrSpks::Spks(_) => false,
        }
    }

    /// Whether this descriptor is a AMP0 descriptor
    #[cfg(feature = "amp0")]
    pub fn is_amp0(&self) -> bool {
        self.is_amp0
    }

    /// Mark the descriptor as not AMP0
    ///
    /// Calling this function improperly might lead to loss of funds.
    /// Do not call this function unless you know what you are doing.
    ///
    /// The chance that someone will actually need this function is
    /// extremely unlikely.
    #[cfg(feature = "amp0")]
    pub fn dangerous_this_wallet_is_not_amp0(&mut self) {
        self.is_amp0 = false;
    }
}

// try to parse as multiline descriptor as exported in green
fn parse_multiline(desc: &str) -> Option<WolletDescriptor> {
    let lines: Vec<_> = desc.trim().split('\n').collect();
    if lines.len() != 2 {
        return None;
    }
    let first_str = lines[0].trim();
    let second_str = lines[1].trim();
    let first = ConfidentialDescriptor::<DescriptorPublicKey>::from_str(first_str);
    let second = ConfidentialDescriptor::<DescriptorPublicKey>::from_str(second_str);
    if first.is_err() || second.is_err() {
        return None;
    }
    let first_no_chk = remove_checksum_if_any(first_str);
    let second_no_chk = remove_checksum_if_any(second_str);
    if first_no_chk.replace("/0/*", "/1/*") != second_no_chk {
        return None;
    }
    let combined = first_no_chk.replace("/0/*", "/<0;1>/*");
    WolletDescriptor::from_str(&combined).ok()
}

fn remove_checksum_if_any(s: &str) -> String {
    s.split('#')
        .next()
        .map(|e| e.to_string())
        .expect("even if '#' is not there we always have one element")
}

fn to_bitcoin_descriptor(s: &str) -> String {
    let s = remove_checksum_if_any(&s[2..]);
    let c = elements_miniscript::descriptor::checksum::desc_checksum(&s).unwrap_or("".into());
    format!("{s}#{c}")
}

/// Simple URL encoding for common characters found in Bitcoin descriptors
pub(crate) fn url_encode_descriptor(desc: &str) -> String {
    desc.chars()
        .map(|c| match c {
            '(' => "%28".to_string(),
            ')' => "%29".to_string(),
            '[' => "%5B".to_string(),
            ']' => "%5D".to_string(),
            '{' => "%7B".to_string(),
            '}' => "%7D".to_string(),
            '/' => "%2F".to_string(),
            '*' => "%2A".to_string(),
            ',' => "%2C".to_string(),
            '%' => "%25".to_string(),
            '&' => "%26".to_string(),
            '#' => "%23".to_string(),
            '+' => "%2B".to_string(),
            '\'' => "%27".to_string(),
            '<' => "%3C".to_string(),
            '>' => "%3E".to_string(),
            ';' => "%3B".to_string(),
            ' ' => "%20".to_string(),
            c => c.to_string(),
        })
        .collect()
}

#[cfg(test)]
mod test {
    use std::{
        collections::hash_map::DefaultHasher,
        hash::{Hash, Hasher},
        str::FromStr,
    };

    use elements::bitcoin;
    use elements_miniscript::{BtcDescriptor, BtcMiniscript, BtcSegwitv0};

    use crate::{
        descriptor::{remove_checksum_if_any, url_encode_descriptor},
        Chain, WolletDescriptor, EC,
    };

    #[track_caller]
    fn chain(desc: &str, expected: Option<Chain>) {
        let wallet_desc = WolletDescriptor::from_str(desc).unwrap();
        let desc = wallet_desc.descriptor().unwrap();
        let chain = Chain::try_from(desc);
        match expected {
            Some(expected) => assert_eq!(chain.unwrap(), expected),
            None => assert!(chain.is_err()),
        };
    }

    #[test]
    fn test_url_encode_descriptor() {
        // Test basic descriptor encoding
        let desc = "ct(slip77(key),elwpkh(xpub/*))";
        let encoded = url_encode_descriptor(desc);
        assert_eq!(encoded, "ct%28slip77%28key%29%2Celwpkh%28xpub%2F%2A%29%29");

        // Test descriptor with brackets
        let desc_with_brackets = "ct(key,elwpkh([fingerprint/path]xpub/*))";
        let encoded_brackets = url_encode_descriptor(desc_with_brackets);
        assert_eq!(
            encoded_brackets,
            "ct%28key%2Celwpkh%28%5Bfingerprint%2Fpath%5Dxpub%2F%2A%29%29"
        );

        // Test descriptor with curly braces
        let desc_with_braces = "ct({key},elwpkh(xpub/*))";
        let encoded_braces = url_encode_descriptor(desc_with_braces);
        assert_eq!(encoded_braces, "ct%28%7Bkey%7D%2Celwpkh%28xpub%2F%2A%29%29");

        // Test descriptor with multipath syntax
        let desc_with_multipath = "ct(key,elwpkh(xpub/<0;1>/*))";
        let encoded_multipath = url_encode_descriptor(desc_with_multipath);
        assert_eq!(
            encoded_multipath,
            "ct%28key%2Celwpkh%28xpub%2F%3C0%3B1%3E%2F%2A%29%29"
        );

        // Test normal characters (should remain unchanged)
        let normal = "ctabc123def";
        assert_eq!(url_encode_descriptor(normal), "ctabc123def");
    }

    #[test]
    fn test_chain_from_descriptor() {
        let blinding = "slip77(ab5824f4477b4ebb00a132adfd8eb0b7935cf24f6ac151add5d1913db374ce92)";
        let base = "elwpkh([759db348/84'/1'/0']tpubDCRMaF33e44pcJj534LXVhFbHibPbJ5vuLhSSPFAw57kYURv4tzXFL6LSnd78bkjqdmE3USedkbpXJUPA1tdzKfuYSL7PianceqAhwL2UkA";

        let desc_multi = format!("ct({blinding},{base}/<0;1>/*))");
        chain(&desc_multi, None);

        let desc_single = format!("ct({blinding},{base}/0/*))");
        chain(&desc_single, Some(Chain::External));

        let desc_single_internal = format!("ct({blinding},{base}/1/*))");
        chain(&desc_single_internal, Some(Chain::Internal));

        let desc_single_internal = format!("ct({blinding},{base}/2/*))");
        chain(&desc_single_internal, None);

        let desc_no_wildcard = format!("ct({blinding},{base}/0/506))");
        chain(&desc_no_wildcard, Some(Chain::External));

        let desc_no_wildcard = format!("ct({blinding},{base}/1/506))");
        chain(&desc_no_wildcard, Some(Chain::Internal));

        let desc_no_wildcard = format!("ct({blinding},{base}/2/506))");
        chain(&desc_no_wildcard, None);
    }

    #[test]
    fn test_wollet_hash() {
        let desc_str = "ct(slip77(ab5824f4477b4ebb00a132adfd8eb0b7935cf24f6ac151add5d1913db374ce92),elwpkh([759db348/84'/1'/0']tpubDCRMaF33e44pcJj534LXVhFbHibPbJ5vuLhSSPFAw57kYURv4tzXFL6LSnd78bkjqdmE3USedkbpXJUPA1tdzKfuYSL7PianceqAhwL2UkA/<0;1>/*))#cch6wrnp";
        let desc: WolletDescriptor = desc_str.parse().unwrap();
        assert_eq!(desc_str, desc.to_string());
        let mut hasher = DefaultHasher::new();
        desc.hash(&mut hasher);
        assert_eq!(12055616352728229988, hasher.finish());
    }

    #[test]
    fn test_dwid() {
        let desc_str = "ct(slip77(ab5824f4477b4ebb00a132adfd8eb0b7935cf24f6ac151add5d1913db374ce92),elwpkh([759db348/84'/1'/0']tpubDCRMaF33e44pcJj534LXVhFbHibPbJ5vuLhSSPFAw57kYURv4tzXFL6LSnd78bkjqdmE3USedkbpXJUPA1tdzKfuYSL7PianceqAhwL2UkA/<0;1>/*))#cch6wrnp";
        let desc: WolletDescriptor = desc_str.parse().unwrap();
        let dwid = desc.dwid(lwk_common::Network::LocaltestLiquid).unwrap();
        assert_eq!(dwid, "384f-8fef-d726-5584-b6b1-88e5-af2e-ce22");

        // Test using SwSigner with the "abandon abandon..." mnemonic
        let mnemonic = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";

        for network in [
            lwk_common::Network::Liquid,
            lwk_common::Network::LocaltestLiquid,
            lwk_common::Network::TestnetLiquid,
        ] {
            let signer = lwk_signer::SwSigner::new(mnemonic, network.is_mainnet()).unwrap();

            // Generate the descriptor using singlesig_desc
            let desc_str = lwk_common::singlesig_desc(
                &signer,
                lwk_common::Singlesig::Wpkh,
                lwk_common::DescriptorBlindingKey::Slip77,
            )
            .unwrap();

            let abandon_desc: WolletDescriptor = desc_str.parse().unwrap();
            let abandon_dwid = abandon_desc.dwid(network).unwrap();

            match network {
                lwk_common::Network::Liquid => {
                    assert_eq!(desc_str, "ct(slip77(9c8e4f05c7711a98c838be228bcb84924d4570ca53f35fa1c793e58841d47023),elwpkh([73c5da0a/84h/1776h/0h]xpub6CRFzUgHFDaiDAQFNX7VeV9JNPDRabq6NYSpzVZ8zW8ANUCiDdenkb1gBoEZuXNZb3wPc1SVcDXgD2ww5UBtTb8s8ArAbTkoRQ8qn34KgcY/<0;1>/*))#y8jljyxl","1");
                    assert_eq!(abandon_dwid, "d41f-fe12-4da1-28d5-9449-5e49-d3f4-42ca", "1");
                }
                lwk_common::Network::LocaltestLiquid => {
                    assert_eq!(desc_str, "ct(slip77(9c8e4f05c7711a98c838be228bcb84924d4570ca53f35fa1c793e58841d47023),elwpkh([73c5da0a/84h/1h/0h]tpubDC8msFGeGuwnKG9Upg7DM2b4DaRqg3CUZa5g8v2SRQ6K4NSkxUgd7HsL2XVWbVm39yBA4LAxysQAm397zwQSQoQgewGiYZqrA9DsP4zbQ1M/<0;1>/*))#xte2lx9x","2");
                    assert_eq!(abandon_dwid, "2cb9-6c92-93b8-1f96-0c3f-afc7-9504-afdd", "2");
                }
                lwk_common::Network::TestnetLiquid => {
                    assert_eq!(desc_str, "ct(slip77(9c8e4f05c7711a98c838be228bcb84924d4570ca53f35fa1c793e58841d47023),elwpkh([73c5da0a/84h/1h/0h]tpubDC8msFGeGuwnKG9Upg7DM2b4DaRqg3CUZa5g8v2SRQ6K4NSkxUgd7HsL2XVWbVm39yBA4LAxysQAm397zwQSQoQgewGiYZqrA9DsP4zbQ1M/<0;1>/*))#xte2lx9x","3");
                    assert_eq!(abandon_dwid, "5f42-ad62-d515-96f8-ed85-5d5f-0e86-467e", "3");
                }
            }
        }
    }

    #[test]
    fn test_is_elip151() {
        let desc_str = "ct(slip77(ab5824f4477b4ebb00a132adfd8eb0b7935cf24f6ac151add5d1913db374ce92),elwpkh([759db348/84'/1'/0']tpubDCRMaF33e44pcJj534LXVhFbHibPbJ5vuLhSSPFAw57kYURv4tzXFL6LSnd78bkjqdmE3USedkbpXJUPA1tdzKfuYSL7PianceqAhwL2UkA/<0;1>/*))#cch6wrnp";
        let desc: WolletDescriptor = desc_str.parse().unwrap();
        assert!(!desc.is_elip151());
        let desc_str = "ct(elip151,elwsh(multi(3,[e6b7814d/87h/1h/0h]tpubDDmvBugC5YMK3UDKjcym7ED8Vfv8aLiX83Tcbecc783VFPEDqBigmzF52uFMyh89bXaf7jAporM1LcoaMcLdKeV4m7ixNAchpMQCL569Ldv/<0;1>/*,[a5a0841e/87h/1h/0h]tpubDDZCCwQJyHksYEfUHb59Mr4ZCo1ndMt4Ys8rXF7RLhmfttU9AYybscFyCmWRVQUxffjGYQe8dtmGchA91PhLUCkH3H7D7Nx1CJLrv5W9tTs/<0;1>/*,[996febb0/87h/1h/0h]tpubDDR1DaDYEKDCkuZ6eU6orfUZshJDDZNjauQsKeK6SkwqPSnRYRPGuYM5MnCqJo88Az8YX6a9oP45W4fenTyS9kLg1TG3LJBbY1jS36r893V/<0;1>/*,[aa15e1ca/87h/1h/0h]tpubDDsijcL7DGGbS2gckSw23LJYiX5s8XDXy4TnYEe7itCMC9LyooBtAVQCaQygQu7Q3yv91NCiCDaYVkrFTrqdM4QY97kFZFHN1ei72B7EcRt/<0;1>/*,[eb4ab844/87h/1h/0h]tpubDDavnLY6q7YwA7eRKRurU8vqjMmXoFF248HQCSGqkApAjKJ4jee8uVVDzeZLedygPRDGorB22F2qjDfdEJmoxGyrgmZ5vDFWBSbSPCgNos5/<0;1>/*)))#vl32vc09";
        let desc: WolletDescriptor = desc_str.parse().unwrap();
        assert!(desc.is_elip151());
        let desc_str = "ct(a45210d9afc904e522bd17a433518d75c6a00cc09ced714b7ec211abdebcb783,elwsh(multi(3,[e6b7814d/87'/1'/0']tpubDDmvBugC5YMK3UDKjcym7ED8Vfv8aLiX83Tcbecc783VFPEDqBigmzF52uFMyh89bXaf7jAporM1LcoaMcLdKeV4m7ixNAchpMQCL569Ldv/<0;1>/*,[a5a0841e/87'/1'/0']tpubDDZCCwQJyHksYEfUHb59Mr4ZCo1ndMt4Ys8rXF7RLhmfttU9AYybscFyCmWRVQUxffjGYQe8dtmGchA91PhLUCkH3H7D7Nx1CJLrv5W9tTs/<0;1>/*,[996febb0/87'/1'/0']tpubDDR1DaDYEKDCkuZ6eU6orfUZshJDDZNjauQsKeK6SkwqPSnRYRPGuYM5MnCqJo88Az8YX6a9oP45W4fenTyS9kLg1TG3LJBbY1jS36r893V/<0;1>/*,[aa15e1ca/87'/1'/0']tpubDDsijcL7DGGbS2gckSw23LJYiX5s8XDXy4TnYEe7itCMC9LyooBtAVQCaQygQu7Q3yv91NCiCDaYVkrFTrqdM4QY97kFZFHN1ei72B7EcRt/<0;1>/*,[eb4ab844/87'/1'/0']tpubDDavnLY6q7YwA7eRKRurU8vqjMmXoFF248HQCSGqkApAjKJ4jee8uVVDzeZLedygPRDGorB22F2qjDfdEJmoxGyrgmZ5vDFWBSbSPCgNos5/<0;1>/*)))#tj07evpd";
        let desc: WolletDescriptor = desc_str.parse().unwrap();
        assert!(desc.is_elip151());
    }

    #[test]
    fn test_strip() {
        let desc_str = "ct(slip77(ab5824f4477b4ebb00a132adfd8eb0b7935cf24f6ac151add5d1913db374ce92),elwpkh([759db348/84'/1'/0']tpubDCRMaF33e44pcJj534LXVhFbHibPbJ5vuLhSSPFAw57kYURv4tzXFL6LSnd78bkjqdmE3USedkbpXJUPA1tdzKfuYSL7PianceqAhwL2UkA/<0;1>/*))#cch6wrnp";
        let desc: WolletDescriptor = desc_str.parse().unwrap();
        let expected = "elwpkh(tpubDCRMaF33e44pcJj534LXVhFbHibPbJ5vuLhSSPFAw57kYURv4tzXFL6LSnd78bkjqdmE3USedkbpXJUPA1tdzKfuYSL7PianceqAhwL2UkA/<0;1>/*)";
        assert_eq!(
            expected,
            desc.bitcoin_descriptor_without_key_origin().unwrap()
        );
    }

    #[test]
    fn parse_fedpegscript() {
        let s = bitcoin::ScriptBuf::from_hex(lwk_test_util::FED_PEG_SCRIPT).unwrap();
        assert_eq!(&s.to_asm_string(), lwk_test_util::FED_PEG_SCRIPT_ASM);

        type Segwitv0Script = BtcMiniscript<bitcoin::PublicKey, BtcSegwitv0>;

        let m = Segwitv0Script::parse(&s).unwrap();
        assert_eq!(m.encode(), s);

        let d = BtcDescriptor::<_>::new_wsh(m).unwrap();

        assert_eq!(&d.to_string(), lwk_test_util::FED_PEG_DESC);
    }

    #[test]
    fn test_parse_multiline() {
        // from green wallet
        let first = "ct(slip77(460830d85d4b299a9406c5899748354937c81b6fdb94f110f8729c9ba2994412),elwpkh([28b3f14e/84'/1'/0']tpubDC2Q4xK4XH72GM7MowNuajyWVbigRLBWKswyP5T88hpPwu5nGqJWnda8zhJEFt71av73Hm8mUMMFSz9acNVzz8b1UbdSHCDXKTbSv5eEytu/0/*))#srt8g93f";
        let second = "ct(slip77(460830d85d4b299a9406c5899748354937c81b6fdb94f110f8729c9ba2994412),elwpkh([28b3f14e/84'/1'/0']tpubDC2Q4xK4XH72GM7MowNuajyWVbigRLBWKswyP5T88hpPwu5nGqJWnda8zhJEFt71av73Hm8mUMMFSz9acNVzz8b1UbdSHCDXKTbSv5eEytu/1/*))#9z93s6yk";
        let both = format!("{first}\n{second}");
        let desc_parsed = WolletDescriptor::from_str_relaxed(&both).unwrap();
        let expected_multi_path = "ct(slip77(460830d85d4b299a9406c5899748354937c81b6fdb94f110f8729c9ba2994412),elwpkh([28b3f14e/84'/1'/0']tpubDC2Q4xK4XH72GM7MowNuajyWVbigRLBWKswyP5T88hpPwu5nGqJWnda8zhJEFt71av73Hm8mUMMFSz9acNVzz8b1UbdSHCDXKTbSv5eEytu/<0;1>/*))#gj65e6vr";
        assert_eq!(desc_parsed.to_string(), expected_multi_path);

        let first_no_cks = remove_checksum_if_any(first);
        let second_no_cks = remove_checksum_if_any(second);
        let first_no_cks_exp = "ct(slip77(460830d85d4b299a9406c5899748354937c81b6fdb94f110f8729c9ba2994412),elwpkh([28b3f14e/84'/1'/0']tpubDC2Q4xK4XH72GM7MowNuajyWVbigRLBWKswyP5T88hpPwu5nGqJWnda8zhJEFt71av73Hm8mUMMFSz9acNVzz8b1UbdSHCDXKTbSv5eEytu/0/*))";
        assert_eq!(first_no_cks, first_no_cks_exp);
        let both_no_checksum = format!("{first_no_cks}\n{second_no_cks}");
        let desc_parsed = WolletDescriptor::from_str_relaxed(&both_no_checksum).unwrap();
        assert_eq!(desc_parsed.to_string(), expected_multi_path);

        let fail_first_desc = format!("{first}X\n{second}");
        assert!(WolletDescriptor::from_str_relaxed(&fail_first_desc).is_err());

        let fail_more_lines = format!("{first}\n{second}\nciao");
        assert!(WolletDescriptor::from_str_relaxed(&fail_more_lines).is_err());

        let second_non_canonical = second.replace("/1/*", "/2/*");
        let fail_more_lines = format!("{first}\n{second_non_canonical}");
        assert!(WolletDescriptor::from_str_relaxed(&fail_more_lines).is_err());
    }

    #[test]
    fn test_is_mainnet() {
        let tpub = "tpubDC2Q4xK4XH72GM7MowNuajyWVbigRLBWKswyP5T88hpPwu5nGqJWnda8zhJEFt71av73Hm8mUMMFSz9acNVzz8b1UbdSHCDXKTbSv5eEytu";
        let xpub = "xpub661MyMwAqRbcFtXgS5sYJABqqG9YLmC4Q1Rdap9gSE8NqtwybGhePY2gZ29ESFjqJoCu1Rupje8YtGqsefD265TMg7usUDFdp6W1EGMcet8";
        let view_key = "1111111111111111111111111111111111111111111111111111111111111111";

        // An xpub might contain the tpub string and viceversa
        let xpub_t = "xpub661MyMwAqRbcH4oCG7tpubMCYWM3pHRZbhBQgi7uVZGcu1EuuomWqwB5gGHXk4VykarKGVA2jKtT4esCXspWW45mzwAzZEsi3U5j94gCKXc";
        let tpub_x = "tpubDC2Q4xK4XH72Gjagbdie6QxG9NpNbgBFzcKBmDL8218u8TSn7WWTBpYPJxpubXVHiLyS8qxPqLCVdu6WGiSDruERaZxusx37LDX5sSkLtrm";

        // testnet/regtest
        for d in [
            format!("ct({view_key},elwpkh({tpub}/*))"),
            format!("ct({view_key},elwpkh({tpub}/<0;1>/*))"),
            format!("ct({view_key},elwsh(multi(2,{tpub}/*,{tpub}/*)))"),
            format!("ct({view_key},elwsh(multi(2,{tpub}/<0;1>/*,{tpub}/<0;1>/*)))"),
            format!("ct({view_key},elwsh(multi(2,{tpub}/*,{xpub}/*)))"),
            format!("ct({view_key},elwpkh({tpub_x}/*))"),
        ] {
            assert!(!WolletDescriptor::from_str(&d).unwrap().is_mainnet());
        }

        // mainnet
        for d in [
            format!("ct({view_key},elwpkh({xpub}/*))"),
            format!("ct({view_key},elwpkh({xpub}/<0;1>/*))"),
            format!("ct({view_key},elwsh(multi(2,{xpub}/*,{xpub}/*)))"),
            format!("ct({view_key},elwsh(multi(2,{xpub}/<0;1>/*,{xpub}/<0;1>/*)))"),
            format!("ct({view_key},elwpkh({xpub_t}/*))"),
        ] {
            assert!(WolletDescriptor::from_str(&d).unwrap().is_mainnet());
        }
    }

    #[test]
    fn test_btc_desc() {
        let keyorigin = "[28b3f14e/84'/1'/0']";
        let xpub = "tpubDC2Q4xK4XH72GM7MowNuajyWVbigRLBWKswyP5T88hpPwu5nGqJWnda8zhJEFt71av73Hm8mUMMFSz9acNVzz8b1UbdSHCDXKTbSv5eEytu";
        let d = format!("ct(elip151,elwpkh({keyorigin}{xpub}/<0;1>/*))");
        let d = WolletDescriptor::from_str(&d).unwrap();
        let ds = d.single_bitcoin_descriptors().unwrap();
        assert_eq!(ds[0], format!("wpkh({keyorigin}{xpub}/0/*)#vgjcw353"));
        assert_eq!(ds[1], format!("wpkh({keyorigin}{xpub}/1/*)#auhenyyf"));
    }

    #[test]
    fn test_wollet_desc_deriv() {
        let keyorigin = "[28b3f14e/84'/1'/0']";
        let xpub = "tpubDC2Q4xK4XH72GM7MowNuajyWVbigRLBWKswyP5T88hpPwu5nGqJWnda8zhJEFt71av73Hm8mUMMFSz9acNVzz8b1UbdSHCDXKTbSv5eEytu";
        let d = format!("ct(elip151,elwpkh({keyorigin}{xpub}/<0;1>/*))");
        let d = WolletDescriptor::from_str(&d).unwrap();
        let params = &elements::AddressParams::ELEMENTS;

        let a = d.address(1, params).unwrap().script_pubkey();
        let s = d.script_pubkey(Chain::External, 1).unwrap();
        assert_eq!(a, s);

        let a = d.change(2, params).unwrap().script_pubkey();
        let s = d.script_pubkey(Chain::Internal, 2).unwrap();
        assert_eq!(a, s);
    }

    #[test]
    fn get_pegin_address() {
        let d: BtcDescriptor<bitcoin::PublicKey> =
            BtcDescriptor::<bitcoin::PublicKey>::from_str(lwk_test_util::FED_PEG_DESC).unwrap();

        let desc: WolletDescriptor = lwk_test_util::PEGIN_TEST_DESC.parse().unwrap();

        let desc_vec = desc
            .descriptor()
            .unwrap()
            .clone()
            .into_single_descriptors()
            .unwrap();
        let pegin = elements_miniscript::descriptor::pegin::Pegin::new(
            d.clone(),
            desc_vec[0].derived_descriptor(&EC, 0).unwrap(),
        );
        let pegin_script = pegin.bitcoin_witness_script(&EC).unwrap();
        let pegin_address = bitcoin::Address::p2wsh(&pegin_script, bitcoin::Network::Testnet);

        let expected = lwk_test_util::PEGIN_TEST_ADDR;
        assert_eq!(pegin_address.to_string(), expected);

        let pegin_address_api = desc.pegin_address(0, bitcoin::Network::Testnet, d).unwrap();
        assert_eq!(pegin_address_api.to_string(), expected);
    }

    #[test]
    fn test_liquid_multisig_issuance() {
        // generated from https://github.com/Blockstream/liquid_multisig_issuance/blob/d0e473871bb7c096dff2baa00f5331022bd7730f/3of5/create_multisig.py
        let json = r#"
        {
                "blindingkey": "1f8ddeeebc82f6b34d8242f635d70ecff0fa61d23899906cf315ecf90662833e",
                "kind": "multisig 3 of 5",
                "multisig": "Azpk4cdN1i62AmgMehkLju97mBc5TCPCPboxpmf5ju8SyAc9Tji3pPCdeyT7cWDnyKBFEij5vgFWjF1M",
                "participants": [
                    {
                        "address": "Azpk4cdN1i62AmgMehkLju97mBc5TCPCPboxpmf5ju8SyAcCUqqE5soNdSRmpnsuEDJmHEEVHEjJRW82",
                        "name": "first",
                        "pubkey": "02d07923f1980be3f3540d1c97241df57786b2c91d2513a059caebcf8fbe47c17d"
                    },
                    {
                        "address": "AzpmxSKSbGLXNYTveF1n5ZUTESQtGWYty4NQvncN64siaWXV7QCvDMHYDDjsRUuGTDTCb2W69yz3nJBU",
                        "name": "second",
                        "pubkey": "02e803b7d19e7fb6286d4c9a8938e777fcffeb444a1795b9919ad3fd34898f2a97"
                    },
                    {
                        "address": "Azpnrg3ezz7SWAzhQXBVKsmiUG1tXq5uRBceZfAC8reJnA4zmRGjegS2d7YuL75ySFHnGwdPUCgNXvmU",
                        "name": "third",
                        "pubkey": "02873ddb31b6a87e775e3e35bf038b3179d496b5164cd8afb57547df9eef5d90b8"
                    },
                    {
                        "address": "AzpjGErNCb8r92MzpospU3zeXQZG2hhr5uVmi7HfeGbQkf3FXhErJgSHdVdFthN1PtV9FBZteHh1t6dz",
                        "name": "fourth",
                        "pubkey": "0337fa5c53d4c9311a079c18e01ec5552eb60a5a34d4a83c921ff6a91050f33dcb"
                    },
                    {
                        "address": "Azpqn9u25NqM5eCpgGM5LGztGWhabfdyfnJKKsVaE4XM12FnUMYtsKr7cDzgQqfhkGCWtGgwCM2Trsan",
                        "name": "fifth",
                        "pubkey": "025f83f10c091e99e4c8e6683f0d0d2c10ee2f2d648d88d2ddd1d2c3cea51460df"
                    }
                ]
            }
        "#;

        #[derive(serde::Deserialize, Debug)]
        struct Participant {
            // address: String,
            // name: String,
            pubkey: String,
        }

        #[derive(serde::Deserialize, Debug)]
        struct WalletMultisig {
            blindingkey: String,
            // kind: String,
            multisig: String,
            participants: Vec<Participant>,
        }

        let v = serde_json::from_str::<WalletMultisig>(json).unwrap();
        let p = &v.participants;
        // println!("{:?}", v);
        let desc = format!(
            "ct({},elsh(multi(3,{},{},{},{},{})))",
            v.blindingkey, p[0].pubkey, p[1].pubkey, p[2].pubkey, p[3].pubkey, p[4].pubkey
        );
        assert_eq!(desc, "ct(1f8ddeeebc82f6b34d8242f635d70ecff0fa61d23899906cf315ecf90662833e,elsh(multi(3,02d07923f1980be3f3540d1c97241df57786b2c91d2513a059caebcf8fbe47c17d,02e803b7d19e7fb6286d4c9a8938e777fcffeb444a1795b9919ad3fd34898f2a97,02873ddb31b6a87e775e3e35bf038b3179d496b5164cd8afb57547df9eef5d90b8,0337fa5c53d4c9311a079c18e01ec5552eb60a5a34d4a83c921ff6a91050f33dcb,025f83f10c091e99e4c8e6683f0d0d2c10ee2f2d648d88d2ddd1d2c3cea51460df)))");
        let desc: elements_miniscript::ConfidentialDescriptor<bitcoin::PublicKey> =
            desc.parse().unwrap();

        let params = &elements::AddressParams::ELEMENTS;
        let a = desc.address(&EC, params).unwrap();
        let expected_address = elements::Address::from_str(&v.multisig).unwrap();

        let mut unconfidential_address = a.to_unconfidential();
        assert_eq!(
            unconfidential_address.to_string(),
            expected_address.to_unconfidential().to_string(),
        );

        assert_ne!(
            a.to_string(),
            expected_address.to_string(),
            "addresses in ct descriptor are always tweaked, thus they are not equal"
        );

        // creating the address by using the blinding key directly
        let blinding_key = bitcoin::secp256k1::SecretKey::from_str(&v.blindingkey).unwrap();
        unconfidential_address.blinding_pubkey = Some(blinding_key.public_key(&EC));
        assert_eq!(
            unconfidential_address.to_string(),
            expected_address.to_string()
        );
    }

    #[test]
    fn test_spks() {
        let key = "0000000000000000000000000000000000000000000000000000000000000001";
        let spk_a = "0014000000000000000000000000000000000000000a";
        let spk_b = "0014000000000000000000000000000000000000000b";
        let params = &elements::AddressParams::ELEMENTS;

        // Mixed blinding key / no blinding key, roundtrip
        let input = format!("{key}:{spk_a},{key}:{spk_b},:{spk_a}");
        let desc = WolletDescriptor::from_str(&input).unwrap();
        assert_eq!(desc.to_string(), input);
        assert_eq!(desc, WolletDescriptor::from_str(&desc.to_string()).unwrap());

        // Serde roundtrip
        let json = serde_json::to_string(&desc).unwrap();
        assert_eq!(
            desc,
            serde_json::from_str::<WolletDescriptor>(&json).unwrap()
        );

        // script_pubkey returns the spk at the given index, ignoring chain
        let expected_a = elements::Script::from_str(spk_a).unwrap();
        let expected_b = elements::Script::from_str(spk_b).unwrap();
        assert_eq!(desc.script_pubkey(Chain::External, 0).unwrap(), expected_a);
        assert_eq!(desc.script_pubkey(Chain::Internal, 0).unwrap(), expected_a);
        assert_eq!(desc.script_pubkey(Chain::External, 1).unwrap(), expected_b);
        assert_eq!(desc.script_pubkey(Chain::External, 2).unwrap(), expected_a);

        // address/change return the same address (no internal/external distinction)
        assert_eq!(
            desc.address(0, params).unwrap(),
            desc.change(0, params).unwrap()
        );

        // With vs without blinding key produce different addresses (index 0 vs 2 share spk_a)
        assert_ne!(
            desc.address(0, params).unwrap(),
            desc.address(2, params).unwrap()
        );

        // IndexOutOfRange
        assert!(desc.script_pubkey(Chain::External, 3).is_err());
        assert!(desc.address(3, params).is_err());
        assert!(desc.change(3, params).is_err());

        // Methods requiring a CT descriptor return Err
        assert!(desc.descriptor().is_err());
        assert!(desc.ct_descriptor().is_err());
        assert!(desc.url_encoded_descriptor().is_err());
        assert!(desc.bitcoin_descriptor_without_key_origin().is_err());
        assert!(desc.definite_descriptor(Chain::External, 0).is_err());
        assert!(desc.single_bitcoin_descriptors().is_err());
        assert!(desc.as_single_descriptors().is_err());
        assert!(desc.dwid(lwk_common::Network::Liquid).is_err());
        let d: BtcDescriptor<bitcoin::PublicKey> =
            BtcDescriptor::<bitcoin::PublicKey>::from_str(lwk_test_util::FED_PEG_DESC).unwrap();
        assert!(desc.pegin_address(0, bitcoin::Network::Testnet, d).is_err());

        assert!(!desc.is_elip151());
        assert!(!desc.has_wildcard());
        assert!(!desc.is_mainnet()); // No network info
    }

    #[test]
    #[cfg(feature = "amp0")]
    fn test_wollet_desc_amp0() {
        let desc_singlesig = "ct(elip151,elwpkh([28b3f14e/84'/1'/0']tpubDC2Q4xK4XH72GM7MowNuajyWVbigRLBWKswyP5T88hpPwu5nGqJWnda8zhJEFt71av73Hm8mUMMFSz9acNVzz8b1UbdSHCDXKTbSv5eEytu/<0;1>/*))";
        let desc_amp0_testnet = "ct(slip77(64321fcf13c2d181ef890ecaf05e973141aa1805949f566232ea52519b35049f),elsh(wsh(multi(2,[98c379b9/3/64185/12352/25274/48669/37222/21152/54418/37839/49229/51085/38856/63304/40878/27010/17469/34767/52063/13856/53616/54101/58845/33548/844/33726/37617/30217/7805/42254/15959/37011/41009/10546/1]tpubECTFwsAEJdFtrfRn1tj2MGXSWa8Ten6JLhr9wYm9w7Fam8CR3w8z9Lfr3HbWmBYArciw7tDYBafjzBjy56CrebLhFAvpjFu7UJjotAChVse/*,[a803afe3/3'/1']tpubDA9GDAo3JyS2TTixDwx3w6bwZBTani1wvBvh5ckjR7PAyvUGvd7z8sHYtd9wh23ExhUqq3F3p3tgJr68LVJK2fkdqmdhxjeSWy8oP261Q1y/1/*))))#emhdrkmv";
        let desc_amp0_mainnet = "ct(slip77(6f9d2dca0d56955cacf89586073ce9db19d46c91533e3222abf10eb46bf8337f),elsh(wsh(multi(2,[0557d83a/3/54195/32530/57583/38568/49379/35784/9512/56310/17245/6737/43041/17998/50002/15170/64436/22872/25420/4993/30612/9196/10098/19/41830/57717/13566/31903/7184/42492/46291/9903/31188/21509/1]xpub7CYN5ZJe3XmqhmNJJJSHP6d36LjKxbKJLNQovGo4wVYSYa4pabxQS2uRMSmzXVC6yD4vaWZGYaq6CuEbc4WPaBRpYYTjHW9BwEjEzn9E4uu/*,[88b6fd7e/3'/1']xpub69mdgvyMbhUaDRbrobUCqAGn9WAWcxKt2oMUxT15EisxCgMAL3CcazyBwPrP7MudQcPbN7VXDyrNQo1pbW1HDhUCzoeJU3giZ2w8crfPe6C/1/*))))#c0c40fkx";

        let wd_singlesig = WolletDescriptor::from_str(desc_singlesig).unwrap();
        let wd_amp0_test = WolletDescriptor::from_str(desc_amp0_testnet).unwrap();
        let wd_amp0_main = WolletDescriptor::from_str(desc_amp0_mainnet).unwrap();

        assert!(!wd_singlesig.is_amp0());
        assert!(wd_amp0_test.is_amp0());
        assert!(wd_amp0_main.is_amp0());

        let params = &elements::AddressParams::ELEMENTS;
        let expected_err =
            "Cannot generate address for AMP0 wallets using this call, use Amp0::address()";
        let err = wd_amp0_test.address(1, params).unwrap_err();
        assert_eq!(err.to_string(), expected_err);
        let err = wd_amp0_main.address(1, params).unwrap_err();
        assert_eq!(err.to_string(), expected_err);

        // For AMP0 Wollet::address() fails too
        use crate::{ElementsNetwork, Wollet};
        let w_amp0_test =
            Wollet::without_persist(ElementsNetwork::LiquidTestnet, wd_amp0_test).unwrap();
        let w_amp0_main = Wollet::without_persist(ElementsNetwork::Liquid, wd_amp0_main).unwrap();

        let err = w_amp0_test.address(Some(1)).unwrap_err();
        assert_eq!(err.to_string(), expected_err);
        let err = w_amp0_main.address(Some(1)).unwrap_err();
        assert_eq!(err.to_string(), expected_err);
    }
}

#[test]
fn test_elip_dwid() {
    // Generate ELIP test vectors with
    // cargo test -p lwk_wollet elip_dwid -- --nocapture
    use lwk_common::{Bip, Network, Signer};
    use lwk_signer::SwSigner;
    let mnemonic = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
    let bip = Bip::Bip84;
    let signer = SwSigner::new(mnemonic, true).unwrap();
    let signer_test = SwSigner::new(mnemonic, false).unwrap();
    let ko_xpub = signer.keyorigin_xpub(bip, true).unwrap();
    let ko_xpub_test = signer_test.keyorigin_xpub(bip, false).unwrap();
    let view = "3e129856c574c66d94023ac98b7f69aca9774d10aee4dc087f0c52a498687189";
    let view_151 = "9fc48763c2aa01aa7b26edb3c8e32f709e4a6b5a83d4379c85864c7e613c5d65";
    let xpub = &ko_xpub[23..];
    let mut i = 0;
    for (descriptor, description, network) in [
        (
            format!("ct({view},elwpkh({ko_xpub}/0/*))"),
            "Liquid",
            Network::Liquid,
        ),
        (
            format!("ct({view},elwpkh({ko_xpub_test}/0/*))"),
            "Liquid Testnet",
            Network::TestnetLiquid,
        ),
        (
            format!("ct({view},elwpkh({ko_xpub_test}/0/*))"),
            "Liquid Regtest",
            Network::LocaltestLiquid,
        ),
        (
            format!("ct({view},elwpkh({xpub}/0/*))"),
            "equivalent descriptor",
            Network::Liquid,
        ),
        (
            format!("ct({view},elwpkh({ko_xpub}/<0;1>/*))"),
            "multi-path",
            Network::Liquid,
        ),
        (
            format!("ct(elip151,elwpkh({ko_xpub}/0/*))"),
            "different blinding key",
            Network::Liquid,
        ),
        (
            format!("ct({view_151},elwpkh({ko_xpub}/0/*))"),
            "equivalent blinding key",
            Network::Liquid,
        ),
        (
            format!("ct({view},elwpkh({ko_xpub}/0))"),
            "no wildcard",
            Network::Liquid,
        ),
        (
            format!("ct({view},elwpkh({ko_xpub}/0/2147483647))"),
            "explicit index 2^31-1",
            Network::Liquid,
        ),
    ] {
        i += 1;
        let d: WolletDescriptor = descriptor.parse().unwrap();
        let dwid = d.dwid(network).unwrap();
        let network_str = match network {
            Network::Liquid => "Liquid",
            Network::TestnetLiquid => "Liquid Testnet",
            Network::LocaltestLiquid => "Liquid Regtest",
        };
        println!("* Test Vector {i}");
        println!("** Description: {description}");
        println!("** Network: {network_str}");
        println!("** CT Descriptor: <code>{descriptor}</code>");
        println!("** DWID: <code>{dwid}</code>\n");
    }
}
