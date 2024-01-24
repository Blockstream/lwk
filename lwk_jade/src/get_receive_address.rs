use serde::{ser::SerializeStruct, Serialize};

#[derive(Debug)]
pub struct GetReceiveAddressParams {
    pub network: crate::Network,

    pub address: SingleOrMulti,
}

/// Singlesig variants for Jade
///
/// Jade supports also legacy pkh but we don't
#[derive(Debug, PartialEq, Eq, Serialize)]
pub enum Variant {
    /// Witness public key hash, BIP84
    #[serde(rename = "wpkh(k)")]
    Wpkh,

    /// Script hash, Witness public key hash AKA nested segwit, BIP49
    #[serde(rename = "sh(wpkh(k))")]
    ShWpkh,
}

#[derive(Debug, PartialEq, Eq)]
pub enum SingleOrMulti {
    Single {
        /// for example "wpkh(k)", not needed if using `multisig_name``
        variant: Variant,
        path: Vec<u32>,
    },
    Multi {
        /// Previously register multisig wallet, cannot be specified with `variant`
        multisig_name: String,
        paths: Vec<Vec<u32>>,
    },
}

impl Serialize for GetReceiveAddressParams {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut state = serializer.serialize_struct("GetReceiveAddressParams", 3)?;
        state.serialize_field("network", &self.network)?;
        match &self.address {
            SingleOrMulti::Single { variant, path } => {
                state.serialize_field("variant", variant)?;
                state.serialize_field("path", path)?;
            }
            SingleOrMulti::Multi {
                multisig_name,
                paths,
            } => {
                state.serialize_field("multisig_name", multisig_name)?;
                state.serialize_field("paths", paths)?;
            }
        }
        state.end()
    }
}

#[cfg(test)]
mod test {
    use serde_json::Value;

    use crate::get_receive_address::{SingleOrMulti, Variant};

    use super::GetReceiveAddressParams;

    #[test]
    fn serialize_get_receive_address() {
        let single_str = r#"
        {
            "network": "liquid",
            "variant": "sh(wpkh(k))",
            "path": [2147483697, 2147483648, 2147483648, 0, 143]
        }
        "#;
        let single_value: Value = serde_json::from_str(single_str).unwrap();
        let single_struct = GetReceiveAddressParams {
            network: crate::Network::Liquid,
            address: SingleOrMulti::Single {
                variant: Variant::ShWpkh,
                path: vec![2147483697, 2147483648, 2147483648, 0, 143],
            },
        };
        assert_eq!(single_value, serde_json::to_value(single_struct).unwrap());

        let multi_str = r#"
        {
            "network": "liquid",
            "multisig_name": "small_beans",
            "paths": [
                [0, 43],
                [0, 14]
            ]
        }
        "#;
        let multi_value: Value = serde_json::from_str(multi_str).unwrap();
        let multi_struct = GetReceiveAddressParams {
            network: crate::Network::Liquid,
            address: SingleOrMulti::Multi {
                multisig_name: "small_beans".to_string(),
                paths: vec![vec![0, 43], vec![0, 14]],
            },
        };
        assert_eq!(multi_value, serde_json::to_value(multi_struct).unwrap());
    }
}
