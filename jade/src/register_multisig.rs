use elements::bitcoin::bip32::ExtendedPubKey;
use serde::{Deserialize, Serialize};

use crate::Network;

#[derive(Debug, Deserialize, Serialize)]
pub struct RegisterMultisigParams {
    pub network: Network,
    pub multisig_name: String, // max 16 chars
    pub descriptor: JadeDescriptor,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct JadeDescriptor {
    pub variant: String, // TODO make enum with segwit types: 'variant' indicates the script type used, and must be one of: 'sh(multi(k))', 'wsh(multi(k))' or 'sh(wsh(multi(k)))'
    pub sorted: bool,
    pub threshold: u32,

    /// The master blinding key in the slip77 format
    #[serde(with = "serde_bytes")]
    pub master_blinding_key: Vec<u8>,

    pub signers: Vec<MultisigSigner>,
}

#[derive(Debug, Deserialize, Serialize)]
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
    use ureq::serde_json;

    use crate::protocol::Request;

    use super::RegisterMultisigParams;

    #[test]
    fn parse_register_multisig() {
        let json = include_str!("../test_data/register_multisig_request.json");

        let _resp: Request<RegisterMultisigParams> = serde_json::from_str(json).unwrap();
    }
}
