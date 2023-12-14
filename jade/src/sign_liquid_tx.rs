use std::fmt::Debug;

use elements::hex::ToHex;
use serde::{ser::SerializeStruct, Deserialize, Serialize};

use crate::{get_receive_address::SingleOrMulti, Network};

#[derive(Serialize)]
pub struct SignLiquidTxParams {
    pub network: Network,

    #[serde(with = "serde_bytes")]
    pub txn: Vec<u8>,

    pub num_inputs: u32,

    pub use_ae_signatures: bool,

    pub change: Vec<Option<Change>>,

    pub asset_info: Vec<AssetInfo>,

    pub trusted_commitments: Vec<Option<Commitment>>,

    pub additional_info: Option<AdditionalInfo>,
}

impl std::fmt::Debug for SignLiquidTxParams {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SignLiquidTxParams")
            .field("network", &self.network)
            .field("txn_bytes", &self.txn.len())
            .field("num_inputs", &self.num_inputs)
            .field("use_ae_signatures", &self.use_ae_signatures)
            .field("change", &self.change)
            .field("asset_info", &self.asset_info)
            .field("trusted_commitments", &self.trusted_commitments)
            .field("additional_info", &self.additional_info)
            .finish()
    }
}

#[derive(Deserialize, Serialize)]
pub struct Commitment {
    #[serde(with = "serde_bytes")]
    pub asset_generator: Vec<u8>,

    #[serde(with = "serde_bytes")]
    pub asset_id: Vec<u8>,

    #[serde(with = "serde_bytes")]
    pub blinding_key: Vec<u8>,

    pub value: u64,

    #[serde(with = "serde_bytes")]
    pub value_commitment: Vec<u8>,

    #[serde(with = "serde_bytes")]
    pub value_blind_proof: Vec<u8>,

    #[serde(with = "serde_bytes")]
    pub asset_blind_proof: Vec<u8>,
}

impl std::fmt::Debug for Commitment {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Commitment")
            .field("asset_generator", &self.asset_generator.to_hex())
            .field("asset_id", &self.asset_id.to_hex())
            .field("blinding_key", &self.blinding_key.to_hex())
            .field("value", &self.value)
            .field("value_commitment", &self.value_commitment.to_hex())
            .field("value_blind_proof", &self.value_blind_proof.to_hex())
            .field("asset_blind_proof", &self.asset_blind_proof.to_hex())
            .finish()
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct Change {
    pub address: SingleOrMulti,
    pub is_change: bool,
}

impl Serialize for Change {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut state = serializer.serialize_struct("Change", 2)?;
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

#[derive(Debug, Deserialize, Serialize)]
pub struct AssetInfo {
    pub asset_id: String,
    pub contract: Contract,
    pub issuance_prevout: Prevout,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Contract {
    pub entity: Entity,

    pub issuer_pubkey: String,
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
    #[serde(with = "serde_bytes")]
    pub asset_id: Vec<u8>,
    pub satoshi: u64,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Prevout {
    pub txid: String,
    pub vout: u32,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Entity {
    pub domain: String,
}

#[derive(Deserialize, Serialize)]
pub struct TxInputParams {
    pub is_witness: bool,

    #[serde(with = "serde_bytes", rename = "script")]
    pub script_code: Vec<u8>,

    #[serde(with = "serde_bytes")]
    pub value_commitment: Vec<u8>,

    pub path: Vec<u32>,

    pub sighash: Option<u32>,

    /// 32 bytes anti-exfiltration commitment (random data not verified for now). TODO verify
    #[serde(with = "serde_bytes")]
    pub ae_host_commitment: Vec<u8>,
}

impl Debug for TxInputParams {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TxInputParams")
            .field("is_witness", &self.is_witness)
            .field("script_code", &self.script_code.to_hex())
            .field("value_commitment", &self.value_commitment.to_hex())
            .field("path", &self.path)
            .field("sighash", &self.sighash)
            .field("ae_host_commitment", &self.ae_host_commitment.to_hex())
            .finish()
    }
}

#[cfg(test)]
mod test {

    use serde_json::Value;

    use crate::get_receive_address::{SingleOrMulti, Variant};

    use super::Change;

    #[test]
    fn parse_change() {
        let json = include_str!("../test_data/sign_liquid_tx_request.json");

        let resp: Value = serde_json::from_str(json).unwrap();

        let params = resp.get("params").unwrap();
        let changes = params.get("change").unwrap();
        let change = changes.get(1).unwrap();

        let expected = Change {
            address: SingleOrMulti::Single {
                variant: Variant::ShWpkh,
                path: vec![2147483697, 2147483648, 2147483648, 0, 143],
            },
            is_change: false,
        };

        assert_eq!(&serde_json::to_value(expected).unwrap(), change);
    }
}
