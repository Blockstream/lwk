use std::str::FromStr;

use bip39::Mnemonic;
use boltz_client::boltz::CreateSubmarineResponse;
use boltz_client::{Bolt11Invoice, Keypair};
use lightning::bitcoin::XKeyIdentifier;
use serde::{Deserialize, Serialize};

use crate::error::Error;
use crate::SwapState;
use crate::SwapType;
use crate::{derive_keypair, mnemonic_identifier};

#[derive(Clone, Debug)]
pub struct PreparePayData {
    pub last_state: SwapState,
    pub swap_type: SwapType,

    /// Fee in satoshi, it's equal to the `amount` less the bolt11 amount
    pub fee: Option<u64>,
    pub bolt11_invoice: Option<Bolt11Invoice>,
    pub create_swap_response: CreateSubmarineResponse,
    pub our_keys: Keypair,
    pub refund_address: String,
    pub key_index: u32,
    pub mnemonic_identifier: XKeyIdentifier,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct PreparePayDataSerializable {
    pub last_state: SwapState,
    pub swap_type: SwapType,
    pub fee: Option<u64>,
    pub bolt11_invoice: Option<String>,
    pub create_swap_response: CreateSubmarineResponse,
    pub key_index: u32,
    pub refund_address: String,

    /// Extended fingerprint of mnemonic used for this boltz swap
    pub mnemonic_identifier: XKeyIdentifier,
}

impl From<PreparePayData> for PreparePayDataSerializable {
    fn from(data: PreparePayData) -> Self {
        PreparePayDataSerializable {
            last_state: data.last_state,
            swap_type: data.swap_type,
            fee: data.fee,
            bolt11_invoice: data.bolt11_invoice.map(|i| i.to_string()),
            create_swap_response: data.create_swap_response,
            key_index: data.key_index,
            refund_address: data.refund_address,
            mnemonic_identifier: data.mnemonic_identifier,
        }
    }
}

pub fn to_prepare_pay_data(
    data: PreparePayDataSerializable,
    mnemonic: &Mnemonic,
) -> Result<PreparePayData, Error> {
    let our_keys = derive_keypair(data.key_index, mnemonic)?;
    let mnemonic_identifier = mnemonic_identifier(mnemonic)?;
    if mnemonic_identifier != data.mnemonic_identifier {
        return Err(Error::MnemonicIdentifierMismatch(
            mnemonic_identifier,
            data.mnemonic_identifier,
        ));
    }
    let bolt11_invoice = data
        .bolt11_invoice
        .as_ref()
        .map(|i| Bolt11Invoice::from_str(i))
        .transpose()?;
    Ok(PreparePayData {
        last_state: data.last_state,
        swap_type: data.swap_type,
        fee: data.fee,
        bolt11_invoice,
        create_swap_response: data.create_swap_response,
        our_keys,
        refund_address: data.refund_address,
        key_index: data.key_index,
        mnemonic_identifier,
    })
}

impl PreparePayDataSerializable {
    pub fn deserialize(data: &str) -> Result<Self, Error> {
        Ok(serde_json::from_str(data)?)
    }
}

#[cfg(test)]
mod tests {
    use boltz_client::ToHex;

    use super::*;

    #[test]
    fn test_prepare_pay_data_serializable() {
        let json_data = include_str!("../tests/data/preapre_pay_data_serializable.json");
        let deserialized: PreparePayDataSerializable = serde_json::from_str(json_data)
            .expect("Failed to deserialize PreparePayDataSerializable from JSON");
        println!("deserialized: {deserialized:?}");
        let mnemonic = Mnemonic::from_str(
            "damp cart merit asset obvious idea chef traffic absent armed road link",
        )
        .unwrap();
        let prepare_pay_data = to_prepare_pay_data(deserialized, &mnemonic).unwrap();
        println!("prepare_pay_data: {prepare_pay_data:?}");
        assert_eq!(
            prepare_pay_data.our_keys.secret_bytes().to_hex(),
            "70f75e954300859f9b32dfea93dfc5667e6cf71d1fad77602d6d6757fd347b01"
        );
    }
}
