use std::sync::Arc;

use elements::hashes::hex::FromHex;

use crate::{types::AssetId, LwkError, TxIn};

/// Wrapper over [`lwk_wollet::Contract`]
#[derive(uniffi::Object)]
#[uniffi::export(Display)]
pub struct Contract {
    inner: lwk_wollet::Contract,
}

impl From<lwk_wollet::Contract> for Contract {
    fn from(inner: lwk_wollet::Contract) -> Self {
        Self { inner }
    }
}

impl From<Contract> for lwk_wollet::Contract {
    fn from(contract: Contract) -> Self {
        contract.inner
    }
}

impl From<&Contract> for lwk_wollet::Contract {
    fn from(contract: &Contract) -> Self {
        contract.inner.clone()
    }
}

impl AsRef<lwk_wollet::Contract> for Contract {
    fn as_ref(&self) -> &lwk_wollet::Contract {
        &self.inner
    }
}

impl std::fmt::Display for Contract {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let json = serde_json::to_string(&self.inner).expect("contain simple types");
        write!(f, "{}", &json)
    }
}

#[uniffi::export]
impl Contract {
    /// Construct a Contract object
    #[uniffi::constructor]
    pub fn new(
        domain: String,
        issuer_pubkey: &str,
        name: String,
        precision: u8,
        ticker: String,
        version: u8,
    ) -> Result<Arc<Self>, LwkError> {
        let inner = lwk_wollet::Contract {
            entity: lwk_wollet::Entity::Domain(domain),
            issuer_pubkey: Vec::<u8>::from_hex(issuer_pubkey)
                .map_err(|e| format!("invalid issuer pubkey: {e}"))?,
            name,
            precision,
            ticker,
            version,
        };
        inner.validate()?; // TODO validate should be the constructor
        Ok(Arc::new(Self { inner }))
    }
}

/// Derive asset id from contract and transaction input
#[uniffi::export]
pub fn derive_asset_id(txin: &TxIn, contract: &Contract) -> Result<AssetId, LwkError> {
    let (asset_id, _token_id) = lwk_wollet::asset_ids(txin.as_ref(), contract.as_ref())?;
    Ok(asset_id.into())
}

/// Derive token id from contract and transaction input
#[uniffi::export]
pub fn derive_token_id(txin: &TxIn, contract: &Contract) -> Result<AssetId, LwkError> {
    let (_asset_id, token_id) = lwk_wollet::asset_ids(txin.as_ref(), contract.as_ref())?;
    Ok(token_id.into())
}
