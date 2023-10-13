use std::{convert::TryFrom, fmt::Display};

use elements::bitcoin::address::WitnessVersion;
use elements_miniscript::{
    confidential::Key,
    descriptor::{DescriptorSecretKey, Wildcard},
    ConfidentialDescriptor, Descriptor, DescriptorPublicKey,
};

#[derive(Debug, Clone)]
/// A wrapper that contains only the subset of CT handled by wollet
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

impl WolletDescriptor {
    pub fn descriptor(&self) -> &Descriptor<DescriptorPublicKey> {
        &self.0.descriptor
    }

    pub fn key(&self) -> &Key<DescriptorPublicKey> {
        &self.0.key
    }
}

impl AsRef<ConfidentialDescriptor<DescriptorPublicKey>> for WolletDescriptor {
    fn as_ref(&self) -> &ConfidentialDescriptor<DescriptorPublicKey> {
        &self.0
    }
}
