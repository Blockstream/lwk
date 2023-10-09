use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
pub struct GetReceiveAddressParams {
    pub network: crate::Network,

    /// Specify when asking a singlesig address. Cannot be specified if `multi` is some.
    #[serde(flatten)]
    pub single: Option<Single>,

    /// Specify when asking a singlesig address. Cannot be specified if `single` is some.
    #[serde(flatten)]
    pub multi: Option<Multi>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Single {
    /// for example "wpkh(k)", not needed if using `multisig_name``
    pub variant: String,
    pub path: Vec<u32>,
}

#[derive(Debug, Deserialize, Serialize)]

pub struct Multi {
    /// Previously register multisig wallet, cannot be specified with `variant`
    pub multisig_name: String,
    pub paths: Vec<Vec<u32>>,
}

#[cfg(test)]
mod test {
    use super::GetReceiveAddressParams;

    #[test]
    fn parse_get_receive_address() {
        let single = r#"
        {
            "network": "liquid",
            "variant": "sh(wpkh(k))",
            "path": [2147483697, 2147483648, 2147483648, 0, 143]
        }
        "#;
        let a: GetReceiveAddressParams = serde_json::from_str(single).unwrap();
        assert_eq!(a.single.unwrap().variant, "sh(wpkh(k))");
        assert!(a.multi.is_none());

        let multi = r#"
        {
            "network": "liquid",
            "multisig_name": "small_beans",
            "paths": [
                [0, 43],
                [0, 14]
            ]
        }
        "#;
        let b: GetReceiveAddressParams = serde_json::from_str(multi).unwrap();
        assert_eq!(b.multi.unwrap().multisig_name, "small_beans");
        assert!(b.single.is_none());
    }
}
