use std::str::FromStr;

use bip39::Mnemonic;
use boltz_client::boltz::CreateReverseResponse;
use boltz_client::util::secrets::Preimage;
use boltz_client::Keypair;
use lightning::bitcoin::XKeyIdentifier;
use lwk_wollet::elements;
use serde::{Deserialize, Serialize};

use crate::derive_keypair;
use crate::error::Error;
use crate::mnemonic_identifier;
use crate::preimage_from_keypair;
use crate::SwapState;
use crate::SwapType;

#[derive(Clone, Debug)]
pub struct InvoiceData {
    pub last_state: SwapState,
    pub swap_type: SwapType,
    pub claim_broadcasted: bool,

    /// The fee of the swap provider if known
    pub fee: Option<u64>,

    pub(crate) create_reverse_response: CreateReverseResponse,
    pub(crate) key_index: u32,
    pub(crate) mnemonic_identifier: XKeyIdentifier,
    pub(crate) our_keys: Keypair,
    pub(crate) preimage: Preimage,
    pub(crate) claim_address: elements::Address,
    pub random_preimage: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct InvoiceDataSerializable {
    pub last_state: SwapState,
    pub swap_type: SwapType,
    pub fee: Option<u64>,
    pub create_reverse_response: CreateReverseResponse,
    pub key_index: u32,
    pub claim_address: elements::Address,

    /// Extended fingerprint of mnemonic used for this boltz swap
    pub mnemonic_identifier: XKeyIdentifier,

    pub claim_broadcasted: Option<bool>, // TODO: remove Option if breaking change

    /// It's some if created at random, otherwise can be derived from mnemonic and key_index
    pub preimage: Option<String>,
}

pub fn to_invoice_data(
    i: InvoiceDataSerializable,
    mnemonic: &Mnemonic,
) -> Result<InvoiceData, Error> {
    let our_keys = derive_keypair(i.key_index, mnemonic)?;
    let preimage = match i.preimage.as_ref() {
        Some(preimage) => Preimage::from_str(preimage)?,
        None => preimage_from_keypair(&our_keys),
    };
    let identifier = mnemonic_identifier(mnemonic)?;
    if identifier != i.mnemonic_identifier {
        return Err(Error::MnemonicIdentifierMismatch(
            identifier,
            i.mnemonic_identifier,
        ));
    }

    Ok(InvoiceData {
        last_state: i.last_state,
        swap_type: i.swap_type,
        fee: i.fee,
        create_reverse_response: i.create_reverse_response,
        our_keys,
        preimage,
        claim_address: i.claim_address,
        key_index: i.key_index,
        mnemonic_identifier: i.mnemonic_identifier,
        claim_broadcasted: i.claim_broadcasted.unwrap_or(false),
        random_preimage: i.preimage.is_some(),
    })
}

impl From<InvoiceData> for InvoiceDataSerializable {
    fn from(i: InvoiceData) -> Self {
        InvoiceDataSerializable {
            last_state: i.last_state,
            swap_type: i.swap_type,
            fee: i.fee,
            create_reverse_response: i.create_reverse_response,
            key_index: i.key_index,
            mnemonic_identifier: i.mnemonic_identifier,
            claim_address: i.claim_address,
            claim_broadcasted: Some(i.claim_broadcasted),
            preimage: i
                .random_preimage
                .then_some(i.preimage.to_string().expect("preimage has bytes")),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::mnemonic_identifier;
    use elements::hex::ToHex;
    use std::str::FromStr;

    use super::*;

    #[test]
    fn test_invoice_data_serializable() {
        let expected_serialized = include_str!("../tests/data/invoice_data_serializable.json");
        // Deserialize the JSON into InvoiceData
        let deserialized: InvoiceDataSerializable =
            serde_json::from_str(expected_serialized).unwrap();

        let mnemonic = Mnemonic::from_str(
            "damp cart merit asset obvious idea chef traffic absent armed road link",
        )
        .unwrap();
        let identifier = mnemonic_identifier(&mnemonic).unwrap();
        assert_eq!(
            identifier.to_string(),
            "e92cd0870c080a91a063345362b7e76d4ad3a4b4"
        );
        let invoice_data = to_invoice_data(deserialized, &mnemonic).unwrap();

        assert_eq!(
            invoice_data.our_keys.secret_bytes().to_hex(),
            "70f75e954300859f9b32dfea93dfc5667e6cf71d1fad77602d6d6757fd347b01"
        );
        assert_eq!(
            invoice_data.preimage.to_string().unwrap(),
            "51db385909dc689c2e93a539d449d753de26e450ec5f5a14e27b8c5e7c25befd".to_string()
        );
    }
}
