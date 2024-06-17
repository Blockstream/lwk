use std::{fmt::Display, str::FromStr};

use aes_gcm_siv::aead::generic_array::GenericArray;
use aes_gcm_siv::aead::NewAead;
use aes_gcm_siv::Aes256GcmSiv;
use elements::bitcoin::{bip32::ChildNumber, WitnessVersion};
use elements::hashes::{sha256t_hash_newtype, Hash};
use elements::{Address, AddressParams};
use elements_miniscript::{
    confidential::Key,
    descriptor::{DescriptorSecretKey, Wildcard},
    ConfidentialDescriptor, Descriptor, DescriptorPublicKey, ForEachKey,
};
use serde::{Deserialize, Serialize};

sha256t_hash_newtype! {
    /// The tag of the hash
    pub struct EncryptionKeyTag = hash_str("LWK-FS-Encryption-Key/1.0");

    /// A tagged hash to generate the key for encryption in the encrypted file system persister
    #[hash_newtype(forward)]
    pub struct EncryptionKeyHash(_);
}

#[derive(Debug, Clone)]
/// A wrapper that contains only the subset of CT descriptors handled by wollet
pub struct WolletDescriptor(ConfidentialDescriptor<DescriptorPublicKey>);

impl Display for WolletDescriptor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        Display::fmt(&self.0, f)
    }
}

impl std::hash::Hash for WolletDescriptor {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.0.to_string().hash(state);
    }
}

impl TryFrom<ConfidentialDescriptor<DescriptorPublicKey>> for WolletDescriptor {
    type Error = crate::error::Error;

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

        if !desc.descriptor.has_wildcard() {
            return Err(Self::Error::UnsupportedDescriptorWithoutWildcard);
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
        match desc.descriptor.desc_type().segwit_version() {
            Some(WitnessVersion::V0) => Ok(WolletDescriptor(desc)),
            _ => Err(Self::Error::UnsupportedDescriptorNonV0),
        }
    }
}

impl FromStr for WolletDescriptor {
    type Err = crate::error::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        ConfidentialDescriptor::<DescriptorPublicKey>::from_str(s)?.try_into()
    }
}

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
        // can keys have different derivation path???
        value.for_each_key(|k| {
            if let Some(path) = k.full_derivation_path() {
                ext_int = path.into_iter().last().cloned();
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

impl WolletDescriptor {
    pub fn descriptor(&self) -> &Descriptor<DescriptorPublicKey> {
        &self.0.descriptor
    }

    /// Return wether the descriptor has a blinding key derived with [Elip151](https://github.com/ElementsProject/ELIPs/blob/main/elip-0151.mediawiki)
    pub fn is_elip151(&self) -> bool {
        if let Ok(elip151_key) = Key::from_elip151(&self.0.descriptor) {
            elip151_key == self.0.key
        } else {
            false
        }
    }

    /// Strip key origin information from the bitcoin descriptor and return it without checksum
    pub fn bitcoin_descriptor_without_key_origin(&self) -> String {
        let desc = self.0.descriptor.to_string();
        let mut result = String::with_capacity(desc.len());
        let mut skip = false;
        for c in desc.chars() {
            if skip {
                if c == ']' {
                    skip = false;
                }
            } else if c == '[' {
                skip = true;
            } else {
                result.push(c)
            }
        }
        result
    }

    /// return the single descriptor if not multipath, if multipath returns the internal or the
    /// external descriptor accordint to `int_or_ext`
    fn inner_descriptor_if_available(&self, ext_int: Chain) -> WolletDescriptor {
        let mut descriptors = self
            .0
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
        WolletDescriptor(ConfidentialDescriptor {
            key: self.0.key.clone(),
            descriptor,
        })
    }

    pub fn change(
        &self,
        index: u32,
        params: &'static AddressParams,
    ) -> Result<Address, crate::error::Error> {
        self.inner_address(index, params, Chain::Internal)
    }

    pub fn cipher(&self) -> Aes256GcmSiv {
        let key_bytes = EncryptionKeyHash::hash(self.to_string().as_bytes()).to_byte_array();
        let key = GenericArray::from_slice(&key_bytes);
        Aes256GcmSiv::new(key)
    }

    pub fn address(
        &self,
        index: u32,
        params: &'static AddressParams,
    ) -> Result<Address, crate::error::Error> {
        self.inner_address(index, params, Chain::External)
    }

    fn inner_address(
        &self,
        index: u32,
        params: &'static AddressParams,
        ext_int: Chain,
    ) -> Result<Address, crate::error::Error> {
        Ok(self
            .inner_descriptor_if_available(ext_int)
            .0
            .at_derivation_index(index)?
            .address(&crate::EC, params)?)
    }

    /// Get a definite descriptor
    pub fn definite_descriptor(
        &self,
        ext_int: Chain,
        index: u32,
    ) -> Result<Descriptor<elements_miniscript::DefiniteDescriptorKey>, crate::Error> {
        let desc = self.inner_descriptor_if_available(ext_int);
        Ok(desc.descriptor().at_derivation_index(index)?)
    }
}

impl AsRef<ConfidentialDescriptor<DescriptorPublicKey>> for WolletDescriptor {
    fn as_ref(&self) -> &ConfidentialDescriptor<DescriptorPublicKey> {
        &self.0
    }
}

#[cfg(test)]
mod test {
    use std::{
        collections::hash_map::DefaultHasher,
        hash::{Hash, Hasher},
    };

    use crate::WolletDescriptor;

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
        let expected = "elwpkh(tpubDCRMaF33e44pcJj534LXVhFbHibPbJ5vuLhSSPFAw57kYURv4tzXFL6LSnd78bkjqdmE3USedkbpXJUPA1tdzKfuYSL7PianceqAhwL2UkA/<0;1>/*)#h8arpn9r";
        assert_eq!(expected, desc.bitcoin_descriptor_without_key_origin());
    }
}
