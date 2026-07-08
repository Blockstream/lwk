use lwk_wollet::elements::AssetId;
use serde::Deserialize;
use uuid::Uuid;

use super::common::OfferStatus;

#[derive(Debug, Clone, Deserialize)]
pub struct OfferListItem {
    pub id: Uuid,
    pub issuance_factory_id: Uuid,
    pub status: OfferStatus,
    pub collateral_asset: AssetId,
    pub principal_asset: AssetId,
    pub collateral_amount: String,
    pub principal_amount: String,
    pub interest_rate: u32,
    pub loan_expiration_height: u32,
    pub created_at_height: u64,
    pub created_at_txid: String,
    pub participants: Vec<ParticipantShort>,
    pub borrower_principal_utxo: Option<OfferUtxoOutpointShort>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ParticipantType {
    Borrower,
    Lender,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ParticipantShort {
    pub participant_type: ParticipantType,
    pub script_pubkey: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct OfferUtxoOutpointShort {
    pub txid: String,
    pub vout: u32,
}
#[derive(Debug, Clone, Deserialize)]
pub struct OfferListResponse {
    pub items: Vec<OfferListItem>,
    pub total: u64,
    pub limit: u64,
    pub offset: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FactoryStatus {
    Active,
    Removed,
}

#[derive(Debug, Clone, Deserialize)]
pub struct FactoryProgramUtxoDto {
    pub txid: String,
    pub vout: u32,
    pub created_at_height: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct FactoryAuthUtxoDto {
    pub txid: String,
    pub vout: u32,
    pub script_pubkey: String,
    pub created_at_height: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct FactoryDetailsResponse {
    pub id: Uuid,
    pub factory_asset_id: String,
    pub program_script_pubkey: String,
    pub status: FactoryStatus,
    pub issuing_utxos_count: u16,
    pub reissuance_flags: u64,
    pub created_at_height: u64,
    pub created_at_txid: String,
    pub auth_utxo: Option<FactoryAuthUtxoDto>,
    pub program_utxo: Option<FactoryProgramUtxoDto>,
}
