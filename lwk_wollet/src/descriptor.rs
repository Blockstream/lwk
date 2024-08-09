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
            } else if c == '[' || c == '#' {
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

    /// Try also to parse it as a non-multipath descriptor specified on 2 lines,
    /// like the format exported by the Green Wallet
    pub fn from_str_relaxed(desc: &str) -> Result<WolletDescriptor, crate::Error> {
        match WolletDescriptor::from_str(desc) {
            Ok(d) => Ok(d),
            Err(e) => parse_multiline(desc).ok_or(e),
        }
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
    let remove_checksum = |e: &str| e.split('#').next().map(|e| e.to_string()).unwrap();
    let first_no_chk = remove_checksum(first_str);
    let second_no_chk = remove_checksum(second_str);
    if first_no_chk.replace("/0/*", "/1/*") != second_no_chk {
        return None;
    }
    let combined = first_no_chk.replace("/0/*", "/<0;1>/*");
    WolletDescriptor::from_str(&combined).ok()
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
        str::FromStr,
    };

    use elements::bitcoin;
    use elements_miniscript::Descriptor;

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
        let expected = "elwpkh(tpubDCRMaF33e44pcJj534LXVhFbHibPbJ5vuLhSSPFAw57kYURv4tzXFL6LSnd78bkjqdmE3USedkbpXJUPA1tdzKfuYSL7PianceqAhwL2UkA/<0;1>/*)";
        assert_eq!(expected, desc.bitcoin_descriptor_without_key_origin());
    }

    #[test]
    fn parse_fedpegscript() {
        // from mainnet @2976295 `elements-cli getsidechaininfo | jq '.current_fedpegscripts[0]'`
        let fedpegscript = "5b21020e0338c96a8870479f2396c373cc7696ba124e8635d41b0ea581112b678172612102675333a4e4b8fb51d9d4e22fa5a8eaced3fdac8a8cbf9be8c030f75712e6af992102896807d54bc55c24981f24a453c60ad3e8993d693732288068a23df3d9f50d4821029e51a5ef5db3137051de8323b001749932f2ff0d34c82e96a2c2461de96ae56c2102a4e1a9638d46923272c266631d94d36bdb03a64ee0e14c7518e49d2f29bc401021031c41fdbcebe17bec8d49816e00ca1b5ac34766b91c9f2ac37d39c63e5e008afb2103079e252e85abffd3c401a69b087e590a9b86f33f574f08129ccbd3521ecf516b2103111cf405b627e22135b3b3733a4a34aa5723fb0f58379a16d32861bf576b0ec2210318f331b3e5d38156da6633b31929c5b220349859cc9ca3d33fb4e68aa08401742103230dae6b4ac93480aeab26d000841298e3b8f6157028e47b0897c1e025165de121035abff4281ff00660f99ab27bb53e6b33689c2cd8dcd364bc3c90ca5aea0d71a62103bd45cddfacf2083b14310ae4a84e25de61e451637346325222747b157446614c2103cc297026b06c71cbfa52089149157b5ff23de027ac5ab781800a578192d175462103d3bde5d63bdb3a6379b461be64dad45eabff42f758543a9645afd42f6d4248282103ed1e8d5109c9ed66f7941bc53cc71137baa76d50d274bda8d5e8ffbd6e61fe9a5fae736402c00fb269522103aab896d53a8e7d6433137bbba940f9c521e085dd07e60994579b64a6d992cf79210291b7d0b1b692f8f524516ed950872e5da10fb1b808b5a526dedc6fed1cf29807210386aa9372fbab374593466bc5451dc59954e90787f08060964d95c87ef34ca5bb53ae68";
        let s = elements::Script::from_str(fedpegscript).unwrap();
        let expected = "OP_PUSHNUM_11 OP_PUSHBYTES_33 020e0338c96a8870479f2396c373cc7696ba124e8635d41b0ea581112b67817261 OP_PUSHBYTES_33 02675333a4e4b8fb51d9d4e22fa5a8eaced3fdac8a8cbf9be8c030f75712e6af99 OP_PUSHBYTES_33 02896807d54bc55c24981f24a453c60ad3e8993d693732288068a23df3d9f50d48 OP_PUSHBYTES_33 029e51a5ef5db3137051de8323b001749932f2ff0d34c82e96a2c2461de96ae56c OP_PUSHBYTES_33 02a4e1a9638d46923272c266631d94d36bdb03a64ee0e14c7518e49d2f29bc4010 OP_PUSHBYTES_33 031c41fdbcebe17bec8d49816e00ca1b5ac34766b91c9f2ac37d39c63e5e008afb OP_PUSHBYTES_33 03079e252e85abffd3c401a69b087e590a9b86f33f574f08129ccbd3521ecf516b OP_PUSHBYTES_33 03111cf405b627e22135b3b3733a4a34aa5723fb0f58379a16d32861bf576b0ec2 OP_PUSHBYTES_33 0318f331b3e5d38156da6633b31929c5b220349859cc9ca3d33fb4e68aa0840174 OP_PUSHBYTES_33 03230dae6b4ac93480aeab26d000841298e3b8f6157028e47b0897c1e025165de1 OP_PUSHBYTES_33 035abff4281ff00660f99ab27bb53e6b33689c2cd8dcd364bc3c90ca5aea0d71a6 OP_PUSHBYTES_33 03bd45cddfacf2083b14310ae4a84e25de61e451637346325222747b157446614c OP_PUSHBYTES_33 03cc297026b06c71cbfa52089149157b5ff23de027ac5ab781800a578192d17546 OP_PUSHBYTES_33 03d3bde5d63bdb3a6379b461be64dad45eabff42f758543a9645afd42f6d424828 OP_PUSHBYTES_33 03ed1e8d5109c9ed66f7941bc53cc71137baa76d50d274bda8d5e8ffbd6e61fe9a OP_PUSHNUM_15 OP_CHECKMULTISIG OP_IFDUP OP_NOTIF OP_PUSHBYTES_2 c00f OP_CSV OP_VERIFY OP_PUSHNUM_2 OP_PUSHBYTES_33 03aab896d53a8e7d6433137bbba940f9c521e085dd07e60994579b64a6d992cf79 OP_PUSHBYTES_33 0291b7d0b1b692f8f524516ed950872e5da10fb1b808b5a526dedc6fed1cf29807 OP_PUSHBYTES_33 0386aa9372fbab374593466bc5451dc59954e90787f08060964d95c87ef34ca5bb OP_PUSHNUM_3 OP_CHECKMULTISIG OP_ENDIF";
        assert_eq!(&s.asm(), expected);

        type Segwitv0Script =
            elements_miniscript::Miniscript<bitcoin::PublicKey, elements_miniscript::Segwitv0>;

        let m = Segwitv0Script::parse(&s).unwrap();
        assert_eq!(m.encode(), s);

        let d = Descriptor::<_, elements_miniscript::NoExt>::new_wsh(m).unwrap();

        // TODO verify it's right
        let expected ="elwsh(or_d(multi(11,020e0338c96a8870479f2396c373cc7696ba124e8635d41b0ea581112b67817261,02675333a4e4b8fb51d9d4e22fa5a8eaced3fdac8a8cbf9be8c030f75712e6af99,02896807d54bc55c24981f24a453c60ad3e8993d693732288068a23df3d9f50d48,029e51a5ef5db3137051de8323b001749932f2ff0d34c82e96a2c2461de96ae56c,02a4e1a9638d46923272c266631d94d36bdb03a64ee0e14c7518e49d2f29bc4010,031c41fdbcebe17bec8d49816e00ca1b5ac34766b91c9f2ac37d39c63e5e008afb,03079e252e85abffd3c401a69b087e590a9b86f33f574f08129ccbd3521ecf516b,03111cf405b627e22135b3b3733a4a34aa5723fb0f58379a16d32861bf576b0ec2,0318f331b3e5d38156da6633b31929c5b220349859cc9ca3d33fb4e68aa0840174,03230dae6b4ac93480aeab26d000841298e3b8f6157028e47b0897c1e025165de1,035abff4281ff00660f99ab27bb53e6b33689c2cd8dcd364bc3c90ca5aea0d71a6,03bd45cddfacf2083b14310ae4a84e25de61e451637346325222747b157446614c,03cc297026b06c71cbfa52089149157b5ff23de027ac5ab781800a578192d17546,03d3bde5d63bdb3a6379b461be64dad45eabff42f758543a9645afd42f6d424828,03ed1e8d5109c9ed66f7941bc53cc71137baa76d50d274bda8d5e8ffbd6e61fe9a),and_v(v:older(4032),multi(2,03aab896d53a8e7d6433137bbba940f9c521e085dd07e60994579b64a6d992cf79,0291b7d0b1b692f8f524516ed950872e5da10fb1b808b5a526dedc6fed1cf29807,0386aa9372fbab374593466bc5451dc59954e90787f08060964d95c87ef34ca5bb))))#gvzs86zz";
        assert_eq!(&d.to_string(), expected);
    }

    #[test]
    fn test_parse_multiline() {
        // from green wallet
        let first = "ct(slip77(460830d85d4b299a9406c5899748354937c81b6fdb94f110f8729c9ba2994412),elwpkh([28b3f14e/84'/1'/0']tpubDC2Q4xK4XH72GM7MowNuajyWVbigRLBWKswyP5T88hpPwu5nGqJWnda8zhJEFt71av73Hm8mUMMFSz9acNVzz8b1UbdSHCDXKTbSv5eEytu/0/*))#srt8g93f";
        let second = "ct(slip77(460830d85d4b299a9406c5899748354937c81b6fdb94f110f8729c9ba2994412),elwpkh([28b3f14e/84'/1'/0']tpubDC2Q4xK4XH72GM7MowNuajyWVbigRLBWKswyP5T88hpPwu5nGqJWnda8zhJEFt71av73Hm8mUMMFSz9acNVzz8b1UbdSHCDXKTbSv5eEytu/1/*))#9z93s6yk";
        let both = format!("{first}\n{second}");
        let desc_parsed = WolletDescriptor::from_str_relaxed(&both).unwrap();
        let expected = "ct(slip77(460830d85d4b299a9406c5899748354937c81b6fdb94f110f8729c9ba2994412),elwpkh([28b3f14e/84'/1'/0']tpubDC2Q4xK4XH72GM7MowNuajyWVbigRLBWKswyP5T88hpPwu5nGqJWnda8zhJEFt71av73Hm8mUMMFSz9acNVzz8b1UbdSHCDXKTbSv5eEytu/<0;1>/*))#gj65e6vr";
        assert_eq!(desc_parsed.to_string(), expected);

        let fail_first_desc = format!("{first}X\n{second}");
        assert!(WolletDescriptor::from_str_relaxed(&fail_first_desc).is_err());

        let fail_more_lines = format!("{first}\n{second}\nciao");
        assert!(WolletDescriptor::from_str_relaxed(&fail_more_lines).is_err());

        let second_non_canonical = second.replace("/1/*", "/2/*");
        let fail_more_lines = format!("{first}\n{second_non_canonical}");
        assert!(WolletDescriptor::from_str_relaxed(&fail_more_lines).is_err());
    }
}
