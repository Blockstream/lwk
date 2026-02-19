use std::str::FromStr;

use bip39::Mnemonic;
use boltz_client::boltz::CreateChainResponse;
use boltz_client::network::{BitcoinChain, Chain, LiquidChain};
use boltz_client::util::secrets::Preimage;
use boltz_client::Keypair;
use lightning::bitcoin::XKeyIdentifier;
use serde::{Deserialize, Serialize};

use crate::error::Error;
use crate::preimage_from_keypair;
use crate::SwapState;
use crate::SwapType;
use crate::{derive_keypair, mnemonic_identifier};

#[derive(Clone, Debug)]
pub struct ChainSwapData {
    pub last_state: SwapState,
    pub swap_type: SwapType,
    pub fee: Option<u64>,
    pub boltz_fee: Option<u64>,
    /// The claim transaction fee estimate from Boltz API (in satoshis)
    /// Used to ensure the actual claim fee matches the quoted fee
    pub claim_fee: Option<u64>,
    pub create_chain_response: CreateChainResponse,
    pub claim_keys: Keypair,
    pub refund_keys: Keypair,
    pub preimage: Preimage,
    pub lockup_address: String,
    pub expected_lockup_amount: u64,
    pub claim_address: String,
    pub refund_address: String,
    pub claim_key_index: u32,
    pub refund_key_index: u32,
    pub mnemonic_identifier: XKeyIdentifier,
    pub from_chain: Chain,
    pub to_chain: Chain,
    pub random_preimage: bool,

    pub claim_txid: Option<String>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ChainSwapDataSerializable {
    pub last_state: SwapState,
    pub swap_type: SwapType,
    pub fee: Option<u64>,
    pub boltz_fee: Option<u64>,
    pub claim_fee: Option<u64>,
    pub create_chain_response: CreateChainResponse,
    pub claim_key_index: u32,
    pub refund_key_index: u32,
    pub lockup_address: String,
    pub expected_lockup_amount: u64,
    pub claim_address: String,
    pub refund_address: String,
    pub mnemonic_identifier: XKeyIdentifier,
    pub from_chain: String,
    pub to_chain: String,

    /// It's some if created at random, otherwise can be derived from mnemonic and key_index
    pub preimage: Option<String>,

    pub claim_txid: Option<String>,
}

impl From<ChainSwapData> for ChainSwapDataSerializable {
    fn from(data: ChainSwapData) -> Self {
        ChainSwapDataSerializable {
            last_state: data.last_state,
            swap_type: data.swap_type,
            fee: data.fee,
            boltz_fee: data.boltz_fee,
            claim_fee: data.claim_fee,
            create_chain_response: data.create_chain_response,
            claim_key_index: data.claim_key_index,
            refund_key_index: data.refund_key_index,
            lockup_address: data.lockup_address,
            expected_lockup_amount: data.expected_lockup_amount,
            claim_address: data.claim_address,
            refund_address: data.refund_address,
            mnemonic_identifier: data.mnemonic_identifier,
            from_chain: data.from_chain.to_string(),
            to_chain: data.to_chain.to_string(),
            preimage: data
                .random_preimage
                .then_some(data.preimage.to_string().expect("preimage has 32 bytes")),
            claim_txid: data.claim_txid,
        }
    }
}

pub(crate) fn chain_from_str(chain: &str) -> Result<Chain, Error> {
    // Display format of the chain is "BTC" or "L-BTC" for regtest/testnet/mainnet
    match chain {
        "BTC" => Ok(Chain::Bitcoin(BitcoinChain::BitcoinRegtest)),
        "L-BTC" => Ok(Chain::Liquid(LiquidChain::LiquidRegtest)),
        s => Err(Error::SwapRestoration(format!("Unknown chain: {s}"))),
    }
}

pub fn to_chain_data(
    data: ChainSwapDataSerializable,
    mnemonic: &Mnemonic,
) -> Result<ChainSwapData, Error> {
    let claim_keys = derive_keypair(data.claim_key_index, mnemonic)?;
    let refund_keys = derive_keypair(data.refund_key_index, mnemonic)?;
    let preimage = match data.preimage.as_ref() {
        Some(preimage) => Preimage::from_str(preimage)?,
        None => preimage_from_keypair(&claim_keys),
    };

    let mnemonic_identifier = mnemonic_identifier(mnemonic)?;
    if mnemonic_identifier != data.mnemonic_identifier {
        return Err(Error::MnemonicIdentifierMismatch(
            mnemonic_identifier,
            data.mnemonic_identifier,
        ));
    }
    let from_chain: Chain = chain_from_str(&data.from_chain)?;
    let to_chain: Chain = chain_from_str(&data.to_chain)?;
    Ok(ChainSwapData {
        last_state: data.last_state,
        swap_type: data.swap_type,
        fee: data.fee,
        boltz_fee: data.boltz_fee,
        claim_fee: data.claim_fee,
        create_chain_response: data.create_chain_response,
        claim_keys,
        refund_keys,
        preimage,
        lockup_address: data.lockup_address,
        expected_lockup_amount: data.expected_lockup_amount,
        claim_address: data.claim_address,
        refund_address: data.refund_address,
        claim_key_index: data.claim_key_index,
        refund_key_index: data.refund_key_index,
        mnemonic_identifier,
        from_chain,
        to_chain,
        random_preimage: data.preimage.is_some(),
        claim_txid: data.claim_txid,
    })
}

impl ChainSwapDataSerializable {
    pub fn deserialize(data: &str) -> Result<Self, Error> {
        Ok(serde_json::from_str(data)?)
    }
}
