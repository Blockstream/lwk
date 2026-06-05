use std::str::FromStr;

use bip39::Mnemonic;
use boltz_client::boltz::CreateSubmarineResponse;
use boltz_client::network::{BitcoinChain, Chain, LiquidChain};
use boltz_client::{Bolt11Invoice, Keypair};
use lightning::bitcoin::XKeyIdentifier;
use lightning::offers::invoice::Bolt12Invoice;
use serde::{Deserialize, Serialize};

use crate::error::Error;
use crate::SwapState;
use crate::SwapType;
use crate::{derive_keypair, mnemonic_identifier};

#[derive(Clone, Debug)]
pub struct PreparePayData {
    pub last_state: SwapState,
    pub swap_type: SwapType,
    pub lockup_txid: Option<String>,
    pub refund_txid: Option<String>,

    /// Fee in satoshi, it's equal to the `amount` less the bolt11 amount
    pub fee: Option<u64>,
    pub boltz_fee: Option<u64>,
    pub bolt11_invoice: Option<Bolt11Invoice>,
    pub bolt12_invoice: Option<Bolt12Invoice>,

    pub create_swap_response: CreateSubmarineResponse,
    pub our_keys: Keypair,
    pub refund_address: String,
    pub key_index: u32,
    pub mnemonic_identifier: XKeyIdentifier,
    pub from_chain: Chain,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct PreparePayDataSerializable {
    pub last_state: SwapState,
    pub swap_type: SwapType,
    pub lockup_txid: Option<String>,
    pub refund_txid: Option<String>,
    pub fee: Option<u64>,
    pub boltz_fee: Option<u64>,
    pub bolt11_invoice: Option<String>,
    pub bolt12_invoice: Option<String>,
    pub create_swap_response: CreateSubmarineResponse,
    pub key_index: u32,
    pub refund_address: String,

    /// Extended fingerprint of mnemonic used for this boltz swap
    pub mnemonic_identifier: XKeyIdentifier,

    #[serde(default = "default_from_chain")]
    pub from_chain: String,
}

impl From<PreparePayData> for PreparePayDataSerializable {
    fn from(data: PreparePayData) -> Self {
        PreparePayDataSerializable {
            last_state: data.last_state,
            swap_type: data.swap_type,
            lockup_txid: data.lockup_txid,
            refund_txid: data.refund_txid,
            fee: data.fee,
            bolt11_invoice: data.bolt11_invoice.map(|i| i.to_string()),
            bolt12_invoice: data
                .bolt12_invoice
                .map(|i| crate::display_bolt12_invoice(&i)),
            create_swap_response: data.create_swap_response,
            key_index: data.key_index,
            refund_address: data.refund_address,
            mnemonic_identifier: data.mnemonic_identifier,
            boltz_fee: data.boltz_fee,
            from_chain: data.from_chain.to_string(),
        }
    }
}

pub fn to_prepare_pay_data(
    data: PreparePayDataSerializable,
    mnemonic: &Mnemonic,
    default_from_chain: Chain,
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
    let bolt12_invoice = data
        .bolt12_invoice
        .as_ref()
        .map(|i| crate::parse_bolt12_invoice(i))
        .transpose()?;
    let from_chain = submarine_chain_from_str(
        &data.from_chain,
        default_from_chain,
        Some(&data.create_swap_response.id),
    )?;
    Ok(PreparePayData {
        last_state: data.last_state,
        swap_type: data.swap_type,
        lockup_txid: data.lockup_txid,
        refund_txid: data.refund_txid,
        fee: data.fee,
        bolt11_invoice,
        bolt12_invoice,
        create_swap_response: data.create_swap_response,
        our_keys,
        refund_address: data.refund_address,
        key_index: data.key_index,
        mnemonic_identifier,
        boltz_fee: data.boltz_fee,
        from_chain,
    })
}

impl PreparePayDataSerializable {
    pub fn deserialize(data: &str) -> Result<Self, Error> {
        Ok(serde_json::from_str(data)?)
    }
}

fn default_from_chain() -> String {
    "L-BTC".to_string()
}

/// `chain` indicates only "BTC" or "L-BTC", we reuse `default_from_chain` inner network
/// to initialize the respective network
pub(crate) fn submarine_chain_from_str(
    chain: &str,
    default_from_chain: Chain,
    swap_id: Option<&str>,
) -> Result<Chain, Error> {
    let liquid_chain = match default_from_chain {
        Chain::Liquid(liquid_chain) => liquid_chain,
        Chain::Bitcoin(_) => {
            return Err(Error::SwapRestoration {
                swap_id: swap_id.map(str::to_owned),
                msg: "Submarine restore expected a Liquid session chain".to_string(),
            })
        }
    };

    match chain {
        "BTC" => Ok(match liquid_chain {
            LiquidChain::Liquid => Chain::Bitcoin(BitcoinChain::Bitcoin),
            LiquidChain::LiquidTestnet => Chain::Bitcoin(BitcoinChain::BitcoinTestnet),
            LiquidChain::LiquidRegtest => Chain::Bitcoin(BitcoinChain::BitcoinRegtest),
        }),
        "L-BTC" => Ok(Chain::Liquid(liquid_chain)),
        s => Err(Error::SwapRestoration {
            swap_id: swap_id.map(str::to_owned),
            msg: format!("Unknown submarine funding chain: {s}"),
        }),
    }
}

#[cfg(test)]
mod tests {
    use boltz_client::network::{Chain, LiquidChain};
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
        assert!(prepare_pay_data.lockup_txid.is_none());
        assert_eq!(
            prepare_pay_data.our_keys.secret_bytes().to_hex(),
            "70f75e954300859f9b32dfea93dfc5667e6cf71d1fad77602d6d6757fd347b01"
        );
    }
}
