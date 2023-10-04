use serde::{Deserialize, Serialize};

use crate::Network;

#[derive(Debug, Deserialize, Serialize)]
pub struct SignLiquidTxParams {
    pub network: Network,

    #[serde(with = "serde_bytes")]
    pub txn: Vec<u8>,

    pub num_inputs: u32,

    pub use_ae_signatures: bool,

    pub change: Vec<Option<Change>>,

    pub asset_info: Vec<AssetInfo>,

    pub trusted_commitments: Vec<Option<Commitment>>,

    pub additional_info: AdditionalInfo,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Commitment {
    pub abf: String,             //hex
    pub asset_generator: String, //hex
    pub asset_id: String,        //hex
    pub blinding_key: String,    //hex
    pub value: u64,

    #[serde(with = "serde_bytes")]
    pub value_commitment: Vec<u8>,

    #[serde(with = "serde_bytes")]
    pub vbf: Vec<u8>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Change {
    pub variant: String,
    pub path: Vec<u32>,
    pub is_change: bool,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct AssetInfo {
    pub asset_id: String, //hex
    pub contract: Contract,
    pub issuance_prevout: Prevout,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Contract {
    pub entity: Entity,
    pub issuer_pubkey: String, //hex
    pub name: String,
    pub precision: u8,
    pub ticker: String,
    pub version: u8,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct AdditionalInfo {
    pub tx_type: String,
    pub wallet_input_summary: Vec<Summary>,
    pub wallet_output_summary: Vec<Summary>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Summary {
    pub asset_id: String, //hex
    pub satoshi: u64,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Prevout {
    pub txid: String, //hex
    pub vout: u32,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Entity {
    pub domain: String,
}

#[cfg(test)]
mod test {
    use ureq::serde_json;

    use crate::protocol::Request;

    use super::SignLiquidTxParams;

    #[test]
    fn parse_sign_liquid_tx() {
        let json = include_str!("../test_data/sign_liquid_tx_request.json");

        let _resp: Request<SignLiquidTxParams> = serde_json::from_str(json).unwrap();
    }
}
