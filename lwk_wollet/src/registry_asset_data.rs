use elements::{AssetId, OutPoint, Transaction, TxIn};

use crate::{asset_ids, Contract, Error};

#[derive(Debug, Clone)]
pub struct RegistryAssetData {
    asset_id: AssetId,
    token_id: AssetId,
    issuance_vin: u32,
    issuance_tx: Transaction,
    contract: Contract,
}

impl RegistryAssetData {
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

    pub fn contract_str(&self) -> String {
        serde_json::to_string(&self.contract).expect("contract")
    }

    pub fn contract(&self) -> &Contract {
        &self.contract
    }

    pub fn issuance_prevout(&self) -> OutPoint {
        self.issuance_tx.input[self.issuance_vin as usize].previous_output
    }

    pub fn reissuance_token(&self) -> AssetId {
        self.token_id
    }

    pub fn token_id(&self) -> AssetId {
        self.token_id
    }

    pub fn asset_id(&self) -> AssetId {
        self.asset_id
    }

    pub fn issuance_tx(&self) -> &Transaction {
        &self.issuance_tx
    }

    pub fn txin(&self) -> &TxIn {
        &self.issuance_tx.input[self.issuance_vin as usize]
    }

    pub fn entropy(&self) -> Result<[u8; 32], Error> {
        let entropy = AssetId::generate_asset_entropy(
            self.txin().previous_output,
            self.contract.contract_hash()?,
        )
        .to_byte_array();
        Ok(entropy)
    }
}
