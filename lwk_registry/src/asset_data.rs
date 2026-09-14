//! Asset data related functions

use std::collections::HashSet;

use crate::contract::{asset_ids, Contract};
use crate::error::Error;
use elements::pset::elip100::{AssetMetadata, TokenMetadata};
use elements::pset::PartiallySignedTransaction;
use elements::{AssetId, OutPoint, Transaction};

/// Add the contracts information of the assets used in the Pset
/// if available in the given `assets` parameter.
/// Without the contract information, the partially signed transaction
/// is valid but will not show asset information when signed with an hardware wallet.
pub fn add_contracts<'a>(
    pset: &mut PartiallySignedTransaction,
    assets: impl Iterator<Item = &'a RegistryAssetData>,
) {
    let assets_in_pset: HashSet<_> = pset.outputs().iter().filter_map(|o| o.asset).collect();
    for registry_data in assets {
        // Policy asset and reissuance tokens do not require the contract
        let asset_id = registry_data.asset_id();
        if assets_in_pset.contains(&asset_id) {
            let metadata = registry_data.asset_metadata();
            pset.add_asset_metadata(asset_id, &metadata);
            let token_id = registry_data.reissuance_token();
            // TODO: handle blinded issuance
            let issuance_blinded = false;
            pset.add_token_metadata(token_id, &TokenMetadata::new(asset_id, issuance_blinded));
        }
    }
}

/// `RegistryAssetData` contains all the data related to an asset with a contract in the registry.
#[derive(Debug, Clone)]
pub struct RegistryAssetData {
    pub(crate) asset_id: AssetId,
    pub(crate) token_id: AssetId,
    pub(crate) issuance_vin: u32,
    pub(crate) issuance_tx: Transaction,
    pub(crate) contract: Contract,
}

impl RegistryAssetData {
    /// Create a new registry asset data from the asset id, the issuance transaction and the contract
    ///
    /// Returns an error if the issuance transaction is not valid for the given asset id and contract
    pub fn new(
        asset_id: AssetId,
        issuance_tx: Transaction,
        contract: Contract,
    ) -> Result<Self, Error> {
        for (vin, txin) in issuance_tx.input.iter().enumerate() {
            let (asset_id_txin, token_id) = txin.issuance_ids();
            if asset_id_txin == asset_id {
                let (asset_id_contract, token_id_contract) = asset_ids(txin, &contract)?;
                if asset_id_contract != asset_id || token_id_contract != token_id {
                    return Err(Error::InvalidContractForAsset(asset_id.to_string()));
                }
                return Ok(Self {
                    asset_id,
                    token_id,
                    issuance_vin: vin as u32,
                    issuance_tx,
                    contract,
                });
            }
        }
        Err(Error::InvalidIssuanceTxtForAsset(asset_id.to_string()))
    }

    /// Get the contract as a string
    pub fn contract_str(&self) -> String {
        serde_json::to_string(&self.contract).expect("contract")
    }

    /// Get the contract
    pub fn contract(&self) -> &Contract {
        &self.contract
    }

    /// Get the issuance transaction prevout
    pub fn issuance_prevout(&self) -> OutPoint {
        self.issuance_tx.input[self.issuance_vin as usize].previous_output
    }

    /// Get the asset id of the reissuance token of this asset id
    pub fn reissuance_token(&self) -> AssetId {
        self.token_id
    }

    /// Get the token id
    pub fn token_id(&self) -> AssetId {
        self.token_id
    }

    /// Get the asset id
    pub fn asset_id(&self) -> AssetId {
        self.asset_id
    }

    /// Get the issuance transaction
    pub fn issuance_tx(&self) -> &Transaction {
        &self.issuance_tx
    }

    /// Get the issuance transaction input
    pub fn txin(&self) -> &elements::TxIn {
        &self.issuance_tx.input[self.issuance_vin as usize]
    }

    /// Get the entropy of the issuance transaction
    pub fn entropy(&self) -> Result<[u8; 32], Error> {
        let entropy = AssetId::generate_asset_entropy(
            self.txin().previous_output,
            self.contract.contract_hash()?,
        )
        .to_byte_array();
        Ok(entropy)
    }

    /// Get the asset metadata from this registry asset data
    pub fn asset_metadata(&self) -> AssetMetadata {
        AssetMetadata::new(self.contract_str(), self.issuance_prevout())
    }
}
