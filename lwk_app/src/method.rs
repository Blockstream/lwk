use std::str::FromStr;

use lwk_rpc_model::{
    request::{self, Direction},
    response,
};
use schemars::schema_for;
use serde_json::Value;

#[derive(Debug, thiserror::Error)]
#[error("The rpc method '{name}' does not exist")]
pub struct MethodNotExist {
    name: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(test, derive(enum_iterator::Sequence))]
pub enum Method {
    Schema,
    Version,
    Scan,
    Stop,
    WalletLoad,
    WalletUnload,
    WalletList,
    WalletDetails,
    WalletAddress,
    WalletBalance,
    WalletUtxos,
    WalletTxs,
    WalletTx,
    WalletSendMany,
    WalletDrain,
    WalletIssue,
    WalletReissue,
    WalletBurn,
    WalletCombine,
    WalletBroadcast,
    WalletPsetDetails,
    WalletMultisigDescriptor,
    WalletSetTxMemo,
    WalletSetAddrMemo,
    LiquidexMake,
    LiquidexTake,
    LiquidexToProposal,
    SignerGenerate,
    SignerJadeId,
    SignerLoadSoftware,
    SignerLoadJade,
    SignerLoadExternal,
    SignerUnload,
    SignerList,
    SignerDetails,
    SignerXpub,
    SignerSign,
    SignerSinglesigDescriptor,
    SignerDeriveBip85,
    SignerRegisterMultisig,
    AssetContract,
    AssetInsert,
    AssetRemove,
    AssetList,
    AssetDetails,
    AssetFromRegistry,
    AssetPublish,
    Amp2Descriptor,
    Amp2Register,
    Amp2Cosign,
}
impl Method {
    pub(crate) fn schema(&self, direction: request::Direction) -> Result<Value, serde_json::Error> {
        serde_json::to_value(match direction {
            Direction::Request => match self {
                Method::Schema => schema_for!(request::Schema),
                Method::Version => schema_for!(request::Empty),
                Method::Scan => schema_for!(request::Empty),
                Method::Stop => schema_for!(request::Empty),
                Method::WalletLoad => schema_for!(request::WalletLoad),
                Method::WalletUnload => schema_for!(request::WalletUnload),
                Method::WalletList => schema_for!(request::Empty),
                Method::WalletDetails => schema_for!(request::WalletDetails),
                Method::WalletAddress => schema_for!(request::WalletAddress),
                Method::WalletBalance => schema_for!(request::WalletBalance),
                Method::WalletUtxos => schema_for!(request::WalletUtxos),
                Method::WalletTxs => schema_for!(request::WalletTxs),
                Method::WalletTx => schema_for!(request::WalletTx),
                Method::WalletSendMany => schema_for!(request::WalletSendMany),
                Method::WalletDrain => schema_for!(request::WalletDrain),
                Method::WalletIssue => schema_for!(request::WalletIssue),
                Method::WalletReissue => schema_for!(request::WalletReissue),
                Method::WalletBurn => schema_for!(request::WalletBurn),
                Method::WalletCombine => schema_for!(request::WalletCombine),
                Method::WalletBroadcast => schema_for!(request::WalletBroadcast),
                Method::WalletPsetDetails => schema_for!(request::WalletPsetDetails),
                Method::WalletMultisigDescriptor => schema_for!(request::WalletMultisigDescriptor),
                Method::WalletSetTxMemo => schema_for!(request::WalletSetTxMemo),
                Method::WalletSetAddrMemo => schema_for!(request::WalletSetAddrMemo),
                Method::LiquidexMake => schema_for!(request::LiquidexMake),
                Method::LiquidexTake => schema_for!(request::LiquidexTake),
                Method::LiquidexToProposal => schema_for!(request::LiquidexToProposal),
                Method::SignerGenerate => schema_for!(request::Empty),
                Method::SignerJadeId => schema_for!(request::Empty),
                Method::SignerLoadSoftware => schema_for!(request::SignerLoadSoftware),
                Method::SignerLoadJade => schema_for!(request::SignerLoadJade),
                Method::SignerLoadExternal => schema_for!(request::SignerLoadExternal),
                Method::SignerUnload => schema_for!(request::SignerUnload),
                Method::SignerList => schema_for!(request::Empty),
                Method::SignerDetails => schema_for!(request::SignerDetails),
                Method::SignerXpub => schema_for!(request::SignerXpub),
                Method::SignerSign => schema_for!(request::SignerSign),
                Method::SignerSinglesigDescriptor => {
                    schema_for!(request::SignerSinglesigDescriptor)
                }
                Method::SignerDeriveBip85 => schema_for!(request::SignerDeriveBip85),
                Method::SignerRegisterMultisig => schema_for!(request::SignerRegisterMultisig),
                Method::AssetContract => schema_for!(request::AssetContract),
                Method::AssetInsert => schema_for!(request::AssetInsert),
                Method::AssetRemove => schema_for!(request::AssetRemove),
                Method::AssetList => schema_for!(request::Empty),
                Method::AssetDetails => schema_for!(request::AssetDetails),
                Method::AssetFromRegistry => schema_for!(request::AssetFromRegistry),
                Method::AssetPublish => schema_for!(request::AssetPublish),
                Method::Amp2Descriptor => schema_for!(request::Amp2Descriptor),
                Method::Amp2Register => schema_for!(request::Amp2Register),
                Method::Amp2Cosign => schema_for!(request::Amp2Cosign),
            },
            Direction::Response => match self {
                Method::Schema => return serde_json::from_str(include_str!("../schema.json")),
                Method::Version => schema_for!(response::Version),
                Method::Scan => schema_for!(response::Empty),
                Method::Stop => schema_for!(request::Empty),
                Method::WalletLoad => schema_for!(response::Wallet),
                Method::WalletUnload => schema_for!(response::WalletUnload),
                Method::WalletList => schema_for!(response::WalletList),
                Method::WalletDetails => schema_for!(response::WalletDetails),
                Method::WalletAddress => schema_for!(response::WalletAddress),
                Method::WalletBalance => schema_for!(response::WalletBalance),
                Method::WalletUtxos => schema_for!(response::WalletUtxos),
                Method::WalletTxs => schema_for!(response::WalletTxs),
                Method::WalletTx => schema_for!(response::WalletTx),
                Method::WalletSendMany => schema_for!(response::Pset),
                Method::WalletDrain => schema_for!(response::Pset),
                Method::WalletIssue => schema_for!(response::Pset),
                Method::WalletReissue => schema_for!(response::Pset),
                Method::WalletBurn => schema_for!(response::Pset),
                Method::WalletCombine => schema_for!(response::WalletCombine),
                Method::WalletBroadcast => schema_for!(response::WalletBroadcast),
                Method::WalletPsetDetails => schema_for!(response::WalletPsetDetails),
                Method::WalletMultisigDescriptor => schema_for!(response::WalletMultisigDescriptor),
                Method::WalletSetTxMemo => schema_for!(response::Empty),
                Method::WalletSetAddrMemo => schema_for!(response::Empty),
                Method::LiquidexMake => schema_for!(response::Pset),
                Method::LiquidexTake => schema_for!(response::Pset),
                Method::LiquidexToProposal => schema_for!(response::LiquidexProposal),
                Method::SignerGenerate => schema_for!(response::SignerGenerate),
                Method::SignerJadeId => schema_for!(response::JadeId),
                Method::SignerLoadSoftware => schema_for!(response::Signer),
                Method::SignerLoadJade => schema_for!(response::Signer),
                Method::SignerLoadExternal => schema_for!(response::Signer),
                Method::SignerUnload => schema_for!(response::SignerUnload),
                Method::SignerList => schema_for!(response::SignerList),
                Method::SignerDetails => schema_for!(response::SignerDetails),
                Method::SignerXpub => schema_for!(response::SignerXpub),
                Method::SignerSign => schema_for!(response::Pset),
                Method::SignerSinglesigDescriptor => {
                    schema_for!(response::SignerSinglesigDescriptor)
                }
                Method::SignerDeriveBip85 => schema_for!(response::SignerDeriveBip85),
                Method::SignerRegisterMultisig => schema_for!(response::Empty),
                Method::AssetContract => schema_for!(response::AssetContract),
                Method::AssetInsert => schema_for!(response::Empty),
                Method::AssetRemove => schema_for!(request::Empty),
                Method::AssetList => schema_for!(response::AssetList),
                Method::AssetDetails => schema_for!(response::AssetDetails),
                Method::AssetFromRegistry => schema_for!(request::Empty),
                Method::AssetPublish => schema_for!(response::AssetPublish),
                Method::Amp2Descriptor => schema_for!(response::Amp2Descriptor),
                Method::Amp2Register => schema_for!(response::Amp2Register),
                Method::Amp2Cosign => schema_for!(response::Amp2Cosign),
            },
        })
    }
}

impl FromStr for Method {
    type Err = MethodNotExist;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "schema" => Method::Schema,
            "version" => Method::Version,
            "scan" => Method::Scan,
            "stop" => Method::Stop,
            "wallet_load" => Method::WalletLoad,
            "wallet_unload" => Method::WalletUnload,
            "wallet_list" => Method::WalletList,
            "wallet_details" => Method::WalletDetails,
            "wallet_address" => Method::WalletAddress,
            "wallet_balance" => Method::WalletBalance,
            "wallet_utxos" => Method::WalletUtxos,
            "wallet_txs" => Method::WalletTxs,
            "wallet_tx" => Method::WalletTx,
            "wallet_send_many" => Method::WalletSendMany,
            "wallet_drain" => Method::WalletDrain,
            "wallet_issue" => Method::WalletIssue,
            "wallet_reissue" => Method::WalletReissue,
            "wallet_burn" => Method::WalletBurn,
            "wallet_combine" => Method::WalletCombine,
            "wallet_broadcast" => Method::WalletBroadcast,
            "wallet_pset_details" => Method::WalletPsetDetails,
            "wallet_multisig_descriptor" => Method::WalletMultisigDescriptor,
            "wallet_set_tx_memo" => Method::WalletSetTxMemo,
            "wallet_set_addr_memo" => Method::WalletSetAddrMemo,
            "liquidex_make" => Method::LiquidexMake,
            "liquidex_take" => Method::LiquidexTake,
            "liquidex_to_proposal" => Method::LiquidexToProposal,
            "signer_generate" => Method::SignerGenerate,
            "signer_jade_id" => Method::SignerJadeId,
            "signer_load_software" => Method::SignerLoadSoftware,
            "signer_load_jade" => Method::SignerLoadJade,
            "signer_load_external" => Method::SignerLoadExternal,
            "signer_unload" => Method::SignerUnload,
            "signer_list" => Method::SignerList,
            "signer_details" => Method::SignerDetails,
            "signer_xpub" => Method::SignerXpub,
            "signer_sign" => Method::SignerSign,
            "signer_singlesig_descriptor" => Method::SignerSinglesigDescriptor,
            "signer_derive_bip85" => Method::SignerDeriveBip85,
            "signer_register_multisig" => Method::SignerRegisterMultisig,
            "asset_contract" => Method::AssetContract,
            "asset_insert" => Method::AssetInsert,
            "asset_remove" => Method::AssetRemove,
            "asset_list" => Method::AssetList,
            "asset_details" => Method::AssetDetails,
            "asset_from_registry" => Method::AssetFromRegistry,
            "asset_publish" => Method::AssetPublish,
            "amp2_descriptor" => Method::Amp2Descriptor,
            "amp2_register" => Method::Amp2Register,
            "amp2_cosign" => Method::Amp2Cosign,
            _ => {
                return Err(MethodNotExist {
                    name: s.to_string(),
                })
            }
        })
    }
}

impl std::fmt::Display for Method {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Method::Schema => "schema",
            Method::Version => "version",
            Method::Scan => "scan",
            Method::Stop => "stop",
            Method::WalletLoad => "wallet_load",
            Method::WalletUnload => "wallet_unload",
            Method::WalletList => "wallet_list",
            Method::WalletDetails => "wallet_details",
            Method::WalletAddress => "wallet_address",
            Method::WalletBalance => "wallet_balance",
            Method::WalletUtxos => "wallet_utxos",
            Method::WalletTxs => "wallet_txs",
            Method::WalletTx => "wallet_tx",
            Method::WalletSendMany => "wallet_send_many",
            Method::WalletDrain => "wallet_drain",
            Method::WalletIssue => "wallet_issue",
            Method::WalletReissue => "wallet_reissue",
            Method::WalletBurn => "wallet_burn",
            Method::WalletCombine => "wallet_combine",
            Method::WalletBroadcast => "wallet_broadcast",
            Method::WalletPsetDetails => "wallet_pset_details",
            Method::WalletMultisigDescriptor => "wallet_multisig_descriptor",
            Method::WalletSetTxMemo => "wallet_set_tx_memo",
            Method::WalletSetAddrMemo => "wallet_set_addr_memo",
            Method::LiquidexMake => "liquidex_make",
            Method::LiquidexTake => "liquidex_take",
            Method::LiquidexToProposal => "liquidex_to_proposal",
            Method::SignerGenerate => "signer_generate",
            Method::SignerJadeId => "signer_jade_id",
            Method::SignerLoadSoftware => "signer_load_software",
            Method::SignerLoadJade => "signer_load_jade",
            Method::SignerLoadExternal => "signer_load_external",
            Method::SignerUnload => "signer_unload",
            Method::SignerList => "signer_list",
            Method::SignerDetails => "signer_details",
            Method::SignerXpub => "signer_xpub",
            Method::SignerSign => "signer_sign",
            Method::SignerSinglesigDescriptor => "signer_singlesig_descriptor",
            Method::SignerDeriveBip85 => "signer_derive_bip85",
            Method::SignerRegisterMultisig => "signer_register_multisig",
            Method::AssetContract => "asset_contract",
            Method::AssetInsert => "asset_insert",
            Method::AssetRemove => "asset_remove",
            Method::AssetList => "asset_list",
            Method::AssetDetails => "asset_details",
            Method::AssetFromRegistry => "asset_from_registry",
            Method::AssetPublish => "asset_publish",
            Method::Amp2Descriptor => "amp2_descriptor",
            Method::Amp2Register => "amp2_register",
            Method::Amp2Cosign => "amp2_cosign",
        };
        write!(f, "{}", s)
    }
}

#[cfg(test)]
mod test {
    use enum_iterator::all;

    use super::Method;

    #[test]
    fn method_roundtrip() {
        let all = all::<Method>().collect::<Vec<_>>();
        for m in all {
            assert_eq!(m, m.to_string().parse().unwrap())
        }
    }
}
