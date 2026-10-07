//! These are temporary conversion utils until the upgrade of `rust-elements` to version 0.27.

use crate::error::ProgramError;

use elements27 as e27;
use lwk_wollet::elements as e25;
use lwk_wollet::hashes::Hash as _;

fn conversion_err<E: std::fmt::Display>(direction: &str, e: E) -> ProgramError {
    ProgramError::Conversion(format!("elements {direction} conversion error: {e}"))
}

pub trait To27<T> {
    fn to27(&self) -> Result<T, ProgramError>;
}

impl<A, T> To27<T> for A
where
    A: e25::encode::Encodable + ?Sized,
    T: e27::encode::Decodable,
{
    fn to27(&self) -> Result<T, ProgramError> {
        e27::encode::deserialize(&e25::encode::serialize(self))
            .map_err(|e| conversion_err("0.25 -> 0.27", e))
    }
}

pub trait To25<T> {
    fn to25(&self) -> Result<T, ProgramError>;
}

impl<A, T> To25<T> for A
where
    A: e27::encode::Encodable + ?Sized,
    T: e25::encode::Decodable,
{
    fn to25(&self) -> Result<T, ProgramError> {
        e25::encode::deserialize(&e27::encode::serialize(self))
            .map_err(|e| conversion_err("0.27 -> 0.25", e))
    }
}

pub fn txout_to27(output: &e25::TxOut) -> Result<e27::TxOut, ProgramError> {
    let mut converted: e27::TxOut = output.to27()?;
    converted.witness = output.witness.to27()?;
    Ok(converted)
}

pub fn txout_secrets_to27(secrets: &e25::TxOutSecrets) -> Result<e27::TxOutSecrets, ProgramError> {
    Ok(e27::TxOutSecrets::new(
        secrets.asset.to27()?,
        e27::confidential::AssetBlindingFactor::from_slice(secrets.asset_bf.into_inner().as_ref())
            .map_err(|e| conversion_err("0.25 -> 0.27 (asset blinding factor)", e))?,
        secrets.value,
        e27::confidential::ValueBlindingFactor::from_slice(secrets.value_bf.into_inner().as_ref())
            .map_err(|e| conversion_err("0.25 -> 0.27 (value blinding factor)", e))?,
    ))
}

pub fn txout_secrets_to25(secrets: &e27::TxOutSecrets) -> Result<e25::TxOutSecrets, ProgramError> {
    Ok(e25::TxOutSecrets::new(
        secrets.asset.to25()?,
        e25::confidential::AssetBlindingFactor::from_slice(secrets.asset_bf.into_inner().as_ref())
            .map_err(|e| conversion_err("0.27 -> 0.25 (asset blinding factor)", e))?,
        secrets.value,
        e25::confidential::ValueBlindingFactor::from_slice(secrets.value_bf.into_inner().as_ref())
            .map_err(|e| conversion_err("0.27 -> 0.25 (value blinding factor)", e))?,
    ))
}

pub fn address_params_to27(network: lwk_common::Network) -> &'static e27::AddressParams {
    match network {
        lwk_common::Network::Liquid => &e27::AddressParams::LIQUID,
        lwk_common::Network::TestnetLiquid => &e27::AddressParams::LIQUID_TESTNET,
        lwk_common::Network::CustomElements(_) => &e27::AddressParams::ELEMENTS,
    }
}

pub fn address_to25(address: &e27::Address) -> Result<e25::Address, ProgramError> {
    address
        .to_string()
        .parse()
        .map_err(|e| conversion_err("0.27 -> 0.25 (address)", e))
}

pub fn leaf_version_to25(
    version: e27::taproot::LeafVersion,
) -> Result<e25::taproot::LeafVersion, ProgramError> {
    e25::taproot::LeafVersion::from_u8(version.as_u8())
        .map_err(|e| conversion_err("0.27 -> 0.25 (leaf version)", e))
}

pub fn sha256_to25(hash: &e27::hashes::sha256::Hash) -> lwk_wollet::hashes::sha256::Hash {
    lwk_wollet::hashes::sha256::Hash::from_byte_array(hash.to_byte_array())
}
