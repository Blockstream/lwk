use elements::bitcoin::bip32::ExtendedPubKey;
use elements_miniscript::{
    confidential::Key, descriptor::WshInner, ConfidentialDescriptor, Descriptor,
    DescriptorPublicKey, Terminal,
};
use serde::{Deserialize, Serialize};

use crate::{derivation_path_to_vec, Network};

#[derive(Debug, Deserialize, Serialize)]
pub struct RegisterMultisigParams {
    pub network: Network,
    pub multisig_name: String, // max 16 chars
    pub descriptor: JadeDescriptor,
}

#[derive(Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct JadeDescriptor {
    pub variant: String, // only 'wsh(multi(k))' supported for now
    pub sorted: bool,
    pub threshold: u32,

    /// The slip77 master blinding key
    #[serde(with = "serde_bytes")]
    pub master_blinding_key: Vec<u8>,

    pub signers: Vec<MultisigSigner>,
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    // #[error(transparent)]
    // Ser(#[from] ciborium::ser::Error<std::io::Error>),
    #[error("Only slip77 master blinding key are supported")]
    OnlySlip77Supported,

    #[error("Only xpub keys are not supported")]
    OnlyXpubKeysAreSupported,

    #[error("Unsupported descriptor type, only wsh is supported")]
    UnsupportedDescriptorType,

    #[error("Unsupported descriptor variant, only multi or sortedmulti are supported")]
    UnsupportedDescriptorVariant,
}

impl TryFrom<&ConfidentialDescriptor<DescriptorPublicKey>> for JadeDescriptor {
    type Error = Error;

    fn try_from(desc: &ConfidentialDescriptor<DescriptorPublicKey>) -> Result<Self, Self::Error> {
        let variant = "wsh(multi(k))".to_string(); // only supported one for now
        let master_blinding_key = match desc.key {
            Key::Slip77(k) => k.as_bytes().to_vec(),
            _ => return Err(Error::OnlySlip77Supported),
        };
        let sorted;
        let threshold;
        let mut signers = vec![];
        match &desc.descriptor {
            Descriptor::Wsh(s) => match s.as_inner() {
                WshInner::SortedMulti(x) => {
                    threshold = x.k as u32;
                    sorted = true;

                    for pk in x.pks.iter() {
                        signers.push(pk.try_into()?);
                    }
                }
                WshInner::Ms(x) => {
                    sorted = false;

                    if let Terminal::Multi(t, keys) = &x.node {
                        threshold = *t as u32;
                        for pk in keys {
                            signers.push(pk.try_into()?);
                        }
                    } else {
                        return Err(Error::UnsupportedDescriptorVariant);
                    }
                }
            },

            _ => return Err(Error::UnsupportedDescriptorType),
        }
        Ok(JadeDescriptor {
            variant,
            sorted,
            threshold,
            master_blinding_key,
            signers,
        })
    }
}

impl TryFrom<&DescriptorPublicKey> for MultisigSigner {
    type Error = Error;

    fn try_from(value: &DescriptorPublicKey) -> Result<Self, Self::Error> {
        let xpub = match value {
            DescriptorPublicKey::XPub(x) => x.xkey,
            _ => return Err(Error::OnlyXpubKeysAreSupported),
        };
        Ok(MultisigSigner {
            fingerprint: value.master_fingerprint().as_bytes().to_vec(),
            derivation: derivation_path_to_vec(
                &value
                    .full_derivation_path()
                    .ok_or(Error::OnlyXpubKeysAreSupported)?,
            ),
            xpub,
            path: vec![],
        })
    }
}

#[derive(Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct MultisigSigner {
    #[serde(with = "serde_bytes")]
    pub fingerprint: Vec<u8>,

    /// From the master node (m) to the xpub
    pub derivation: Vec<u32>,

    pub xpub: ExtendedPubKey,

    /// From the xpub to the signer
    pub path: Vec<u32>,
}

#[cfg(test)]
mod test {
    use elements_miniscript::{ConfidentialDescriptor, DescriptorPublicKey};

    use crate::{protocol::Request, register_multisig::MultisigSigner};

    use super::{JadeDescriptor, RegisterMultisigParams};

    #[test]
    fn parse_register_multisig() {
        let json = include_str!("../test_data/register_multisig_request.json");

        let _resp: Request<RegisterMultisigParams> = serde_json::from_str(json).unwrap();
    }

    #[test]
    fn from_desc_to_jade_desc() {
        let a= "tpubDDCNstnPhbdd4vwbw5UWK3vRQSF1WXQkvBHpNXpKJAkwFYjwu735EH3GVf53qwbWimzewDUv68MUmRDgYtQ1AU8FRCPkazfuaBp7LaEaohG";
        let b: &str = "tpubDDExQpZg2tziZ7ACSBCYsY3rYxAZtTRBgWwioRLYqgNBguH6rMHN1D8epTxUQUB5kM5nxkEtr2SNic6PJLPubcGMR6S2fmDZTzL9dHpU7ka";
        let slip77_key = "9c8e4f05c7711a98c838be228bcb84924d4570ca53f35fa1c793e58841d47023";
        let kind = ["sortedmulti", "multi"];
        for t in 1..=2 {
            for k in kind {
                let desc = format!("ct(slip77({slip77_key}),elwsh({k}({t},{a}/*,{b}/*)))");
                let desc: ConfidentialDescriptor<DescriptorPublicKey> = desc.parse().unwrap();

                let jade_desc: JadeDescriptor = (&desc).try_into().unwrap();

                assert_eq!(
                    jade_desc,
                    JadeDescriptor {
                        variant: "wsh(multi(k))".to_string(),
                        sorted: k == "sortedmulti",
                        threshold: t,
                        master_blinding_key: hex::decode(slip77_key).unwrap(),
                        signers: vec![
                            MultisigSigner {
                                fingerprint: vec![146, 26, 57, 253],
                                derivation: vec![],
                                xpub: a.parse().unwrap(),
                                path: vec![]
                            },
                            MultisigSigner {
                                fingerprint: vec![195, 206, 35, 178],
                                derivation: vec![],
                                xpub: b.parse().unwrap(),
                                path: vec![]
                            }
                        ]
                    }
                )
            }
        }
    }
}
