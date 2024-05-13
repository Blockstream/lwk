use elements::{
    bitcoin::bip32::{ChildNumber, Fingerprint, Xpub},
    hex::ToHex,
    Script,
};
use elements_miniscript::{
    confidential::Key, descriptor::WshInner, ConfidentialDescriptor, Descriptor,
    DescriptorPublicKey, Terminal,
};
use serde::{Deserialize, Serialize};
use std::str::FromStr;

use crate::{derivation_path_to_vec, Error, Network};

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct GetRegisteredMultisigParams {
    /// Name of the multisig wallet
    ///
    /// Max 16 chars
    pub multisig_name: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct RegisterMultisigParams {
    pub network: Network,

    /// Name of the multisig wallet
    ///
    /// Max 16 chars
    pub multisig_name: String,
    pub descriptor: JadeDescriptor,
}

#[derive(Deserialize, Serialize, PartialEq, Eq, Clone)]
pub struct JadeDescriptor {
    pub variant: String, // only 'wsh(multi(k))' supported for now
    pub sorted: bool,
    pub threshold: u32,

    /// The slip77 master blinding key
    #[serde(with = "serde_bytes")]
    pub master_blinding_key: Vec<u8>,

    pub signers: Vec<MultisigSigner>,
}

impl std::fmt::Debug for JadeDescriptor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("JadeDescriptor")
            .field("variant", &self.variant)
            .field("sorted", &self.sorted)
            .field("threshold", &self.threshold)
            .field("master_blinding_key", &self.master_blinding_key.to_hex())
            .field("signers", &self.signers)
            .finish()
    }
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

impl TryFrom<&JadeDescriptor> for ConfidentialDescriptor<DescriptorPublicKey> {
    type Error = Error;

    fn try_from(desc: &JadeDescriptor) -> Result<Self, Self::Error> {
        if &desc.variant != "wsh(multi(k))" {
            return Err(Error::UnsupportedDescriptorType);
        }
        let sorted = if desc.sorted { "sorted" } else { "" };
        let slip77 = desc.master_blinding_key.to_hex();
        let threshold = desc.threshold;
        let xpubs = desc
            .signers
            .iter()
            .map(|s| s.keyorigin_xpub_multi())
            .collect::<Vec<_>>()
            .join(",");
        let desc = format!("ct(slip77({slip77}),elwsh({sorted}multi({threshold},{xpubs})))");
        Self::from_str(&desc).map_err(|_| Error::UnsupportedDescriptorType)
    }
}

impl JadeDescriptor {
    /// Derive the witness script
    ///
    /// `JadeDescriptor`s returned from Jade's `get_registered_multisig` signers do not have `path`
    /// set. When we need to derive the witness script we need to derive the definite descriptor
    /// which requires the paths from the xpubs.
    /// In this functions the path used for _all_ xpubs is `/is_change/index`.
    pub fn derive_witness_script(&self, is_change: bool, index: u32) -> Result<Script, Error> {
        let mut jade_desc = self.clone();
        for signer in jade_desc.signers.iter_mut() {
            signer.path = vec![if is_change { 1 } else { 0 }];
        }
        let ct_desc: ConfidentialDescriptor<DescriptorPublicKey> = (&jade_desc).try_into()?;
        let def_desc = ct_desc
            .descriptor
            .at_derivation_index(index)
            .map_err(|_| Error::UnsupportedDescriptorType)?;
        def_desc
            .explicit_script()
            .map_err(|_| Error::UnsupportedDescriptorType)
    }
}

impl TryFrom<&DescriptorPublicKey> for MultisigSigner {
    type Error = Error;

    fn try_from(value: &DescriptorPublicKey) -> Result<Self, Self::Error> {
        let (xpub, origin) = match value {
            DescriptorPublicKey::XPub(x) => (x.xkey, x.origin.as_ref()),
            DescriptorPublicKey::MultiXPub(x) => (x.xkey, x.origin.as_ref()),
            DescriptorPublicKey::Single(_) => return Err(Error::SingleKeyAreNotSupported),
        };
        Ok(MultisigSigner {
            fingerprint: value.master_fingerprint(),
            derivation: origin
                .map(|o| derivation_path_to_vec(&o.1))
                .unwrap_or(vec![]),
            xpub,
            path: vec![], // to support multipath we avoid specifying the fixed part here and pass all the path at signing request phase
        })
    }
}

#[derive(Debug, Deserialize, Serialize, PartialEq, Eq, Clone)]
pub struct MultisigSigner {
    pub fingerprint: Fingerprint,

    /// From the master node (m) to the xpub
    pub derivation: Vec<u32>,

    pub xpub: Xpub,

    /// From the xpub to the signer
    pub path: Vec<u32>,
}

fn path_to_string(path: &[u32]) -> String {
    path.iter()
        .map(|e| format!("/{}", ChildNumber::from(*e)))
        .collect::<Vec<_>>()
        .join("")
}

impl MultisigSigner {
    pub fn keyorigin_xpub_multi(&self) -> String {
        let keyorigin = if self.derivation.is_empty() {
            "".to_string()
        } else {
            format!("[{}{}]", self.fingerprint, path_to_string(&self.derivation))
        };
        format!(
            "{keyorigin}{}{}/<0;1>/*",
            self.xpub,
            path_to_string(&self.path)
        )
    }
}

#[derive(Deserialize, Serialize)]
pub struct RegisteredMultisig {
    variant: String,
    sorted: bool,
    threshold: u32,
    pub num_signers: u32,

    #[serde(with = "serde_bytes")]
    master_blinding_key: Vec<u8>,
}

impl std::fmt::Debug for RegisteredMultisig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RegisteredMultisig")
            .field("variant", &self.variant)
            .field("sorted", &self.sorted)
            .field("threshold", &self.threshold)
            .field("num_signers", &self.num_signers)
            .field("master_blinding_key", &self.master_blinding_key.to_hex())
            .finish()
    }
}

#[derive(Debug, Deserialize, Serialize, PartialEq, Eq, Clone)]
pub struct RegisteredMultisigDetails {
    pub multisig_name: String,
    pub descriptor: JadeDescriptor,
}

#[cfg(test)]
mod test {
    use elements::bitcoin::bip32::Fingerprint;
    use elements_miniscript::{ConfidentialDescriptor, DescriptorPublicKey};

    use crate::register_multisig::MultisigSigner;

    use super::{JadeDescriptor, RegisterMultisigParams};

    #[test]
    fn parse_register_multisig() {
        let json = include_str!("../test_data/register_multisig.json");

        let _resp: RegisterMultisigParams = serde_json::from_str(json).unwrap();
    }

    #[test]
    fn from_desc_to_jade_desc() {
        let a= "tpubDDCNstnPhbdd4vwbw5UWK3vRQSF1WXQkvBHpNXpKJAkwFYjwu735EH3GVf53qwbWimzewDUv68MUmRDgYtQ1AU8FRCPkazfuaBp7LaEaohG";
        let b  = "tpubDDExQpZg2tziZ7ACSBCYsY3rYxAZtTRBgWwioRLYqgNBguH6rMHN1D8epTxUQUB5kM5nxkEtr2SNic6PJLPubcGMR6S2fmDZTzL9dHpU7ka";
        let slip77_key = "9c8e4f05c7711a98c838be228bcb84924d4570ca53f35fa1c793e58841d47023";
        let kind = ["sortedmulti", "multi"];
        for t in 1..=2 {
            for k in kind {
                // TODO add keyorigin
                let desc =
                    format!("ct(slip77({slip77_key}),elwsh({k}({t},{a}/<0;1>/*,{b}/<0;1>/*)))");
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
                                fingerprint: Fingerprint::from([146, 26, 57, 253]),
                                derivation: vec![],
                                xpub: a.parse().unwrap(),
                                path: vec![]
                            },
                            MultisigSigner {
                                fingerprint: Fingerprint::from([195, 206, 35, 178]),
                                derivation: vec![],
                                xpub: b.parse().unwrap(),
                                path: vec![]
                            }
                        ]
                    }
                );
                let desc2: ConfidentialDescriptor<DescriptorPublicKey> =
                    (&jade_desc).try_into().unwrap();
                assert_eq!(desc, desc2);
            }
        }
    }
}
