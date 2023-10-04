use serde::{Deserialize, Serialize};

use crate::Network;

#[derive(Debug, Deserialize, Serialize)]
struct SignLiquidTxParams {
    network: Network,

    #[serde(with = "serde_bytes")]
    txn: Vec<u8>,

    num_inputs: u32,

    use_ae_signatures: bool,

    change: Vec<Option<Change>>,

    asset_info: Vec<AssetInfo>,

    trusted_commitments: Vec<Option<Commitment>>,

    additional_info: AdditionalInfo,
}

#[derive(Debug, Deserialize, Serialize)]
struct Commitment {
    abf: String,             //hex
    asset_generator: String, //hex
    asset_id: String,        //hex
    blinding_key: String,    //hex
    value: u64,

    #[serde(with = "serde_bytes")]
    value_commitment: Vec<u8>,

    #[serde(with = "serde_bytes")]
    vbf: Vec<u8>,
}

#[derive(Debug, Deserialize, Serialize)]
struct Change {
    variant: String,
    path: Vec<u32>,
    is_change: bool,
}

#[derive(Debug, Deserialize, Serialize)]
struct AssetInfo {
    asset_id: String, //hex
    contract: Contract,
    issuance_prevout: Prevout,
}

#[derive(Debug, Deserialize, Serialize)]
struct Contract {
    entity: Entity,
    issuer_pubkey: String, //hex
    name: String,
    precision: u8,
    ticker: String,
    version: u8,
}

#[derive(Debug, Deserialize, Serialize)]
struct AdditionalInfo {
    tx_type: String,
    wallet_input_summary: Vec<Summary>,
    wallet_output_summary: Vec<Summary>,
}

#[derive(Debug, Deserialize, Serialize)]
struct Summary {
    asset_id: String, //hex
    satoshi: u64,
}

#[derive(Debug, Deserialize, Serialize)]
struct Prevout {
    txid: String, //hex
    vout: u32,
}

#[derive(Debug, Deserialize, Serialize)]
struct Entity {
    domain: String,
}

#[cfg(test)]
mod test {
    use ureq::serde_json;

    use crate::protocol::Request;

    use super::SignLiquidTxParams;

    #[test]
    fn sign_liquid_tx() {
        let json = include_str!("../test_data/sign_liquid_tx_request.json");

        let _resp: Request<SignLiquidTxParams> = serde_json::from_str(json).unwrap();
    }
}
