use std::str::FromStr;

use bip39::Mnemonic;
use boltz_client::boltz::CreateReverseResponse;
use boltz_client::network::{BitcoinChain, Chain, LiquidChain};
use boltz_client::util::secrets::Preimage;
use boltz_client::Keypair;
use lightning::bitcoin::XKeyIdentifier;
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

    /// The fee of the swap provider and the network fee if known
    pub fee: Option<u64>,

    /// The fee of the swap provider
    pub boltz_fee: Option<u64>,

    /// The claim transaction fee estimate from Boltz API (in satoshis)
    /// Used to ensure the actual claim fee matches the quoted fee
    pub claim_fee: Option<u64>,

    pub claim_txid: Option<String>,

    /// Lockup transaction sent from boltz
    pub lockup_txid: Option<String>,

    pub(crate) create_reverse_response: CreateReverseResponse,
    pub(crate) key_index: u32,
    pub(crate) mnemonic_identifier: XKeyIdentifier,
    pub(crate) our_keys: Keypair,
    pub(crate) preimage: Preimage,
    pub(crate) claim_address: String,
    pub(crate) to_chain: Chain,
    pub random_preimage: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct InvoiceDataSerializable {
    pub last_state: SwapState,
    pub swap_type: SwapType,
    pub fee: Option<u64>,
    pub boltz_fee: Option<u64>,
    pub claim_fee: Option<u64>,
    pub claim_txid: Option<String>,
    pub lockup_txid: Option<String>,
    pub create_reverse_response: CreateReverseResponse,
    pub key_index: u32,
    pub claim_address: String,
    #[serde(default = "default_to_chain")]
    pub to_chain: String,

    /// Extended fingerprint of mnemonic used for this boltz swap
    pub mnemonic_identifier: XKeyIdentifier,

    pub claim_broadcasted: Option<bool>, // TODO: remove Option if breaking change

    /// It's some if created at random, otherwise can be derived from mnemonic and key_index
    pub preimage: Option<String>,
}

pub fn to_invoice_data(
    i: InvoiceDataSerializable,
    mnemonic: &Mnemonic,
    default_to_chain: Chain,
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
        boltz_fee: i.boltz_fee,
        claim_fee: i.claim_fee,
        claim_txid: i.claim_txid,
        lockup_txid: i.lockup_txid,
        create_reverse_response: i.create_reverse_response,
        our_keys,
        preimage,
        claim_address: i.claim_address,
        to_chain: reverse_chain_from_str(&i.to_chain, default_to_chain, None)?,
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
            boltz_fee: i.boltz_fee,
            claim_fee: i.claim_fee,
            claim_txid: i.claim_txid,
            lockup_txid: i.lockup_txid,
            create_reverse_response: i.create_reverse_response,
            key_index: i.key_index,
            mnemonic_identifier: i.mnemonic_identifier,
            claim_address: i.claim_address,
            to_chain: i.to_chain.to_string(),
            claim_broadcasted: Some(i.claim_broadcasted),
            preimage: i
                .random_preimage
                .then_some(i.preimage.to_string().expect("preimage has bytes")),
        }
    }
}

impl InvoiceDataSerializable {
    pub fn deserialize(data: &str) -> Result<Self, Error> {
        Ok(serde_json::from_str(data)?)
    }
}

fn default_to_chain() -> String {
    "L-BTC".to_string()
}

pub(crate) fn reverse_chain_from_str(
    chain: &str,
    default_to_chain: Chain,
    swap_id: Option<&str>,
) -> Result<Chain, Error> {
    // TODO: Blend this with `submarine_chain_from_str` once reverse and submarine
    // chain-aware restore paths settle.
    let liquid_chain = match default_to_chain {
        Chain::Liquid(liquid_chain) => liquid_chain,
        Chain::Bitcoin(_) => {
            return Err(Error::SwapRestoration {
                swap_id: swap_id.map(ToOwned::to_owned),
                msg: "Reverse restore expected a Liquid session chain".to_string(),
            });
        }
    };

    match chain {
        "BTC" => Ok(Chain::Bitcoin(match liquid_chain {
            LiquidChain::Liquid => BitcoinChain::Bitcoin,
            LiquidChain::LiquidTestnet => BitcoinChain::BitcoinTestnet,
            LiquidChain::LiquidRegtest => BitcoinChain::BitcoinRegtest,
        })),
        "L-BTC" => Ok(Chain::Liquid(liquid_chain)),
        s => Err(Error::SwapRestoration {
            swap_id: swap_id.map(ToOwned::to_owned),
            msg: format!("Unknown reverse claim chain: {s}"),
        }),
    }
}

#[cfg(test)]
mod tests {
    use crate::mnemonic_identifier;
    use boltz_client::network::{Chain, LiquidChain};
    use lwk_wollet::elements::hex::ToHex;
    use std::str::FromStr;

    use super::*;

    #[test]
    fn test_invoice_data_serializable() {
        let expected_serialized = include_str!("../tests/data/invoice_data_serializable.json");
        // Deserialize the JSON into InvoiceData
        let deserialized: InvoiceDataSerializable =
            serde_json::from_str(expected_serialized).unwrap();
        assert_eq!(deserialized.to_chain, "L-BTC");

        let mnemonic = Mnemonic::from_str(
            "damp cart merit asset obvious idea chef traffic absent armed road link",
        )
        .unwrap();
        let identifier = mnemonic_identifier(&mnemonic).unwrap();
        assert_eq!(
            identifier.to_string(),
            "e92cd0870c080a91a063345362b7e76d4ad3a4b4"
        );
        let invoice_data = to_invoice_data(
            deserialized,
            &mnemonic,
            Chain::Liquid(LiquidChain::LiquidRegtest),
        )
        .unwrap();

        assert_eq!(
            invoice_data.claim_address.to_string(),
            "el1qqd3yqxz9gu9r8stv5vrhzjaa6d9ks4elkxr92fxp3pxy5m43jjk6h672z708ucn50638ahpc0unxa92uu39h5vypvwzft9r5e"
        );
        assert_eq!(
            invoice_data.to_chain,
            Chain::Liquid(LiquidChain::LiquidRegtest)
        );
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
