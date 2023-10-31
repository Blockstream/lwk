use std::{
    convert::{TryFrom, TryInto},
    fmt::Display,
    str::FromStr,
};

use elements::{bitcoin::address::WitnessVersion, Script};
use elements::{Address, AddressParams};
use elements_miniscript::{
    confidential::Key,
    descriptor::{DescriptorSecretKey, Wildcard},
    ConfidentialDescriptor, Descriptor, DescriptorPublicKey,
};
use pset_common::derive_script_pubkey;

#[derive(Debug, Clone)]
/// A wrapper that contains only the subset of CT descriptors handled by wollet
pub struct WolletDescriptor(ConfidentialDescriptor<DescriptorPublicKey>);

impl Display for WolletDescriptor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        Display::fmt(&self.0, f)
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
            return Err(Self::Error::UnsupportedDescriptorWithMultipath);
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

impl WolletDescriptor {
    pub fn descriptor(&self) -> &Descriptor<DescriptorPublicKey> {
        &self.0.descriptor
    }

    pub fn address(
        &self,
        index: u32,
        params: &'static AddressParams,
    ) -> Result<Address, crate::error::Error> {
        Ok(self
            .0
            .at_derivation_index(index)?
            .address(&crate::EC, params)?)
    }

    pub fn derive_script_pubkey(&self, index: u32) -> Result<Script, crate::error::Error> {
        Ok(derive_script_pubkey(&self.0, index)?)
    }
}

impl AsRef<ConfidentialDescriptor<DescriptorPublicKey>> for WolletDescriptor {
    fn as_ref(&self) -> &ConfidentialDescriptor<DescriptorPublicKey> {
        &self.0
    }
}
