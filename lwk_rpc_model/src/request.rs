//! Data models of every requests made via RPC

use std::net::SocketAddr;

#[cfg(doc)]
use crate::response;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// An empty request, doesn't require any param.
#[derive(JsonSchema)]
pub struct Empty {}

/// Request a JSON schema of a method of the RPC
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct Schema {
    /// Name of the method to request the schema for
    pub method: String,

    /// Specify if requesting the schema for the request or the response
    pub direction: Direction,
}

/// The direction, to the server (request) or from the server (response)
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum Direction {
    /// Request, to the server
    Request,

    /// Response, from the server
    Response,
}

/// Request to load a wallet in the server, returning [`response::Wallet`]
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct WalletLoad {
    /// The read-only descriptor describing the wallet outputs
    pub descriptor: String,

    /// The name given to the wallet, will be needed for calls related to the wallet
    pub name: String,
}

/// Unload the wallet identified by the given name
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct WalletUnload {
    /// The name given to the wallet
    pub name: String,
}

/// Load a signer in the server
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct SignerLoadSoftware {
    /// The name of the signer, will be needed to reference it in other calls
    pub name: String,

    /// The mnemonic (12 or 24 words)
    pub mnemonic: String,

    /// Whether to persist the software signer
    pub persist: bool,
}

/// Load a signer in the server
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct SignerLoadJade {
    /// The name of the signer, will be needed to reference it in other calls
    pub name: String,

    /// Full identifier of the jade
    pub id: String,

    /// If set, instead of looking for physical jade, try to connect to the emulator at the following port
    pub emulator: Option<SocketAddr>,
}

/// Load a signer in the server
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct SignerLoadExternal {
    /// The name of the signer, will be needed to reference it in other calls
    pub name: String,

    /// The fingerprint identifyng the external signer
    pub fingerprint: String,
}

/// Get the signer details
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct SignerDetails {
    /// The name of the signer
    pub name: String,
}

/// Unload the signer identified by the given name
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct SignerUnload {
    /// The name of the signer
    pub name: String,
}

/// Derive a BIP85 mnemonic from a loaded software signer
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct SignerDeriveBip85 {
    /// The name of the signer
    pub name: String,

    /// Index for the derived mnemonic (0-based)
    pub index: u32,

    /// Number of words in the derived mnemonic (12 or 24)
    pub word_count: u32,
}

/// Request a receiving address
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct WalletAddress {
    /// The wallet name
    pub name: String,

    /// The derivation index for the wildcard, if missing the first unused index is used
    pub index: Option<u32>,

    /// The signer name
    pub signer: Option<String>,

    /// Whether to return a QR code of the address encoded as text
    pub with_text_qr: bool,

    /// Return a image QR code encoded as uri with the given pixel per module
    pub with_uri_qr: Option<u8>,
}

/// The balance of a wallet
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct WalletBalance {
    /// The wallet name
    pub name: String,

    /// Replace asset ids with tickers when possible
    pub with_tickers: bool,
}

/// Send a transaction from a wallet
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct WalletSendMany {
    /// The wallet name creating the transaction
    pub name: String,

    /// Recipient addressees
    pub addressees: Vec<UnvalidatedAddressee>,

    /// Optional fee rate in sat/kvB
    pub fee_rate: Option<f32>,
}

///  An addressee which has yet to be validated
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct UnvalidatedAddressee {
    /// The amount to send in satoshi
    pub satoshi: u64,

    /// The address to send to
    ///
    /// If "burn", the output will be burned
    pub address: String,

    /// The asset to send
    ///
    /// If empty, the policy asset
    pub asset: String,
}

/// Drain a wallet
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct WalletDrain {
    /// The wallet name creating the transaction
    pub name: String,

    /// Recipient addresse
    pub address: String,

    /// Optional fee rate in sat/kvB
    pub fee_rate: Option<f32>,
}

/// A request containing information to create a single signature descriptor wallet
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct SignerSinglesigDescriptor {
    /// The signer name
    pub name: String,

    /// The descriptor blinding key
    pub descriptor_blinding_key: String,

    /// The singlesig kind // TODO enum
    pub singlesig_kind: String,
}

/// A request containing information to create a multi signature descriptor wallet
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct WalletMultisigDescriptor {
    /// The descriptor blinding key
    pub descriptor_blinding_key: String,

    /// The multisig kind // TODO enum
    pub multisig_kind: String,

    /// The number of signatures required to spend
    pub threshold: u32,

    /// The partecipants in the multisig wallet xpubs with key origin
    pub keyorigin_xpubs: Vec<String>,
}

/// Request to register a multisig wallet on a signer
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct SignerRegisterMultisig {
    /// The signer name
    pub name: String,

    /// The wallet name
    pub wallet: String,
}

/// Request to a signer for a derived xpub
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct SignerXpub {
    /// The signer name
    pub name: String,

    /// The xpub kind // TODO enum
    pub xpub_kind: String,
}

/// A request to sign a PSET
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct SignerSign {
    /// The signer name
    pub name: String,

    /// The PSET in base64
    pub pset: String,
}

/// Request to broadcast a transaction
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct WalletBroadcast {
    /// The wallet name
    pub name: String,

    /// Perform transaction extraction and verification but avoid doing the last broadcast step // TODO verification is not complete at the moment
    pub dry_run: bool,

    /// The PSET in base64
    pub pset: String,
}

/// Request details for a wallet
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct WalletDetails {
    /// The wallet name
    pub name: String,
}

/// Request to do an issuance
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct WalletIssue {
    /// The wallet name doing the issuance
    pub name: String,

    /// The number of units of the asset created
    pub satoshi_asset: u64,

    /// The address receiving the asset, if missing a receiving address from the wallet doing the issuance is used
    pub address_asset: Option<String>,

    /// The number of reissuance token to be created
    pub satoshi_token: u64,

    /// The address receiving the reissuance token, if missing a receiving address from the wallet doing the issuance is used
    pub address_token: Option<String>,

    /// The contract defininig asset metadata, such as name, ticker and precision. See [`AssetContract`] request to create
    pub contract: Option<String>,

    /// The optional fee rate
    pub fee_rate: Option<f32>,
}

/// Request to do a reissuance
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct WalletReissue {
    /// The wallet name doing the reissuance
    pub name: String,

    /// The asset to reissue
    pub asset: String,

    /// The number of units of the asset created
    pub satoshi_asset: u64,

    /// The address receiving the asset, if missing a receiving address from the wallet doing the reissuance is used
    pub address_asset: Option<String>,

    /// The optional fee rate
    pub fee_rate: Option<f32>,
}

/// Request to do burn an asset
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct WalletBurn {
    /// The wallet name
    pub name: String,

    /// The asset to burn
    pub asset: String,

    /// The number of units of the asset to burn
    pub satoshi_asset: u64,

    /// The optional fee rate
    pub fee_rate: Option<f32>,
}

/// A request creating a contract in the JSON format expected by the issue call
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct AssetContract {
    /// Domain of the issuer
    pub domain: String,

    /// Pubkey of the issuer in hex format (33 bytes/66 chars)
    pub issuer_pubkey: String,

    /// The name of the asset to be created
    pub name: String,

    /// The precision of the amount of the created asset, for example 2 means two digits after the decimal separator
    pub precision: u8,

    /// The ticker of the asset
    pub ticker: String,

    /// The protocol version (0)
    pub version: u8,
}

/// Request to combine PSETs
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct WalletCombine {
    /// The wallet name
    pub name: String,

    /// A list of PSET to combine
    pub pset: Vec<String>,
}

/// Request to see details of a PSET
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct WalletPsetDetails {
    /// The wallet name
    pub name: String,

    /// The PSET in base64 to inspect
    pub pset: String,

    /// Replace asset ids with tickers when possible
    pub with_tickers: bool,
}

/// Request to get the wallet unspet transaction Outputs
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct WalletUtxos {
    /// The wallet name
    pub name: String,
}

/// Request to get the wallet transactions
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct WalletTxs {
    /// The wallet name
    pub name: String,

    /// Replace asset ids with tickers when possible
    pub with_tickers: bool,
}

/// Request to get a transaction
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct WalletTx {
    /// The wallet name
    pub name: String,

    /// Transaction ID
    pub txid: String,

    /// Use the explorer if necessary
    pub from_explorer: bool,
}

/// Request to have details of an asset
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct AssetDetails {
    /// The asset identifier
    pub asset_id: String,
}

/// Request to insert an asset
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct AssetInsert {
    /// Asset ID in hex
    pub asset_id: String,

    /// Contract committed to the asset id
    pub contract: String,

    /// Issuance transaction in hex
    pub issuance_tx: String,
}

/// Request to remove an asset
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct AssetRemove {
    /// The asset identifier
    pub asset_id: String,
}

/// Request to insert an asset retrieving information from the asset registry
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct AssetFromRegistry {
    /// The asset identifier
    pub asset_id: String,
}

/// Request to publish
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct AssetPublish {
    /// The asset identifier
    pub asset_id: String,
}

/// Request to obtain jade identifiers
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct SignerJadeId {
    /// If set, instead of looking for physical jade, try to connect to the emulator at the following port
    pub emulator: Option<SocketAddr>,
}

/// Request to set a transaction memo
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct WalletSetTxMemo {
    /// The wallet name
    pub name: String,

    /// Transaction id
    pub txid: String,

    /// Transaction memo
    pub memo: String,
}

/// Request to set an address memo
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct WalletSetAddrMemo {
    /// The wallet name
    pub name: String,

    /// Address
    pub address: String,

    /// Address memo
    pub memo: String,
}

/// Request to make a Liquidex proposal
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct LiquidexMake {
    /// The wallet name
    pub name: String,

    /// The transaction id of the output to spend
    pub txid: String,

    /// The vout index of the output to spend
    pub vout: u32,

    /// The asset to receive
    pub asset: String,

    /// The amount to receive in satoshi
    pub satoshi: u64,
}

/// Request to take a Liquidex proposal
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct LiquidexTake {
    /// The wallet name
    pub name: String,

    /// The LiquidexProposal JSON to take
    pub proposal: String,
}

/// Request to convert a PSET to a Liquidex proposal
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct LiquidexToProposal {
    /// The PSET
    pub pset: String,
}

/// Request to get an AMP2 wallet descriptor
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct Amp2Descriptor {
    /// The signer name
    pub name: String,
}

/// Request to register an AMP2 wallet descriptor
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct Amp2Register {
    /// The signer name
    pub name: String,
}

/// Request for AMP2 to cosign a PSET
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct Amp2Cosign {
    /// The PSET to cosign
    pub pset: String,
}

#[cfg(test)]
mod test {
    use schemars::schema_for;

    use crate::request::*;

    #[test]
    fn test_json_schema() {
        let schema = schema_for!(WalletLoad);
        assert_eq!(
            r#"{"$schema":"http://json-schema.org/draft-07/schema#","title":"WalletLoad","description":"Request to load a wallet in the server, returning [`response::Wallet`]","type":"object","required":["descriptor","name"],"properties":{"descriptor":{"description":"The read-only descriptor describing the wallet outputs","type":"string"},"name":{"description":"The name given to the wallet, will be needed for calls related to the wallet","type":"string"}}}"#,
            serde_json::to_string(&schema).unwrap()
        );
    }
}
