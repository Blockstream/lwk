use elements::bitcoin::bip32::ExtendedPubKey;
use serde::{Deserialize, Serialize};

use crate::Network;

#[derive(Debug, Deserialize, Serialize)]
pub struct RegisterMultisigParams {
    pub network: Network,
    pub multisig_name: String,
    pub descriptor: JadeDescriptor,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct JadeDescriptor {
    pub variant: String,
    pub sorted: bool,
    pub threshold: u32,

    #[serde(with = "serde_bytes")]
    pub master_blinding_key: Vec<u8>,

    pub signers: Vec<JadeSigner>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct JadeSigner {
    #[serde(with = "serde_bytes")]
    pub fingerprint: Vec<u8>,

    pub derivation: Vec<u32>,

    pub xpub: ExtendedPubKey,

    pub path: Vec<u32>,
}

#[cfg(test)]
mod test {
    use ureq::serde_json;

    use crate::protocol::Request;

    use super::RegisterMultisigParams;

    #[test]
    fn parse_register_multisig() {
        let json = include_str!("../test_data/register_multisig_request.json");

        let _resp: Request<RegisterMultisigParams> = serde_json::from_str(json).unwrap();
    }
}
