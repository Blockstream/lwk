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
    SignerGenerate,
    Version,
    WalletLoad,
    WalletUnload,
    WalletList,
    SignerLoadSoftware,
    SignerLoadJade,
    SignerLoadExternal,
    SignerDetails,
    SignerUnload,
    SignerList,
    WalletAddress,
    WalletBalance,
    WalletSendMany,
    SignerSinglesigDescriptor,
    WalletMultisigDescriptor,
    SignerRegisterMultisig,
    SignerXpub,
    SignerSign,
    WalletBroadcast,
    WalletDetails,
    WalletCombine,
    WalletPsetDetails,
    WalletUtxos,
    WalletTxs,
    WalletSetTxMemo,
    WalletSetAddrMemo,
    WalletIssue,
    WalletReissue,
    AssetContract,
    AssetDetails,
    AssetList,
    AssetInsert,
    AssetRemove,
    AssetPublish,
    Scan,
    Stop,
    SignerJadeId,
}
impl Method {
    pub(crate) fn schema(&self, direction: request::Direction) -> Result<Value, serde_json::Error> {
        serde_json::to_value(match direction {
            Direction::Request => match self {
                Method::Schema => schema_for!(request::Schema),
                Method::SignerGenerate => schema_for!(request::Empty),
                Method::Version => schema_for!(request::Empty),
                Method::WalletLoad => schema_for!(request::WalletLoad),
                Method::WalletUnload => schema_for!(request::WalletUnload),
                Method::WalletList => schema_for!(request::Empty),
                Method::SignerLoadSoftware => schema_for!(request::SignerLoadSoftware),
                Method::SignerLoadJade => schema_for!(request::SignerLoadJade),
                Method::SignerLoadExternal => schema_for!(request::SignerLoadExternal),
                Method::SignerDetails => schema_for!(request::SignerDetails),
                Method::SignerUnload => schema_for!(request::SignerUnload),
                Method::SignerList => schema_for!(request::Empty),
                Method::WalletAddress => schema_for!(request::WalletAddress),
                Method::WalletBalance => schema_for!(request::WalletBalance),
                Method::WalletSendMany => schema_for!(request::WalletSendMany),
                Method::SignerSinglesigDescriptor => schema_for!(request::SignerSinglesigDescriptor),
                Method::WalletMultisigDescriptor => schema_for!(request::WalletMultisigDescriptor),
                Method::SignerRegisterMultisig => schema_for!(request::SignerRegisterMultisig),
                Method::SignerXpub => schema_for!(request::SignerXpub),
                Method::SignerSign => schema_for!(request::SignerSign),
                Method::WalletBroadcast => schema_for!(request::WalletBroadcast),
                Method::WalletDetails => schema_for!(request::WalletDetails),
                Method::WalletCombine => schema_for!(request::WalletCombine),
                Method::WalletPsetDetails => schema_for!(request::WalletPsetDetails),
                Method::WalletUtxos => schema_for!(request::WalletUtxos),
                Method::WalletTxs => schema_for!(request::WalletTxs),
                Method::WalletSetTxMemo => schema_for!(request::WalletSetTxMemo),
                Method::WalletSetAddrMemo => schema_for!(request::WalletSetAddrMemo),
                Method::WalletIssue => schema_for!(request::WalletIssue),
                Method::WalletReissue => schema_for!(request::WalletReissue),
                Method::AssetContract => schema_for!(request::AssetContract),
                Method::AssetDetails => schema_for!(request::AssetDetails),
                Method::AssetList => schema_for!(request::Empty),
                Method::AssetInsert => schema_for!(request::AssetInsert),
                Method::AssetRemove => schema_for!(request::AssetRemove),
                Method::Scan => schema_for!(request::Empty),
                Method::Stop => schema_for!(request::Empty),
                Method::SignerJadeId => schema_for!(request::Empty),
                Method::AssetPublish => schema_for!(request::AssetPublish),
            },
            Direction::Response => match self {
                Method::Schema => return serde_json::from_str(include_str!("../schema.json")),
                Method::SignerGenerate => schema_for!(response::SignerGenerate),
                Method::Version => schema_for!(response::Version),
                Method::WalletLoad => schema_for!(response::Wallet),
                Method::WalletUnload => schema_for!(response::WalletUnload),
                Method::WalletList => schema_for!(response::WalletList),
                Method::SignerLoadSoftware => schema_for!(response::Signer),
                Method::SignerLoadJade => schema_for!(response::Signer),
                Method::SignerLoadExternal => schema_for!(response::Signer),
                Method::SignerDetails => schema_for!(response::SignerDetails),
                Method::SignerUnload => schema_for!(response::SignerUnload),
                Method::SignerList => schema_for!(response::SignerList),
                Method::WalletAddress => schema_for!(response::WalletAddress),
                Method::WalletBalance => schema_for!(response::WalletBalance),
                Method::WalletSendMany => schema_for!(response::Pset),
                Method::SignerSinglesigDescriptor => schema_for!(response::SignerSinglesigDescriptor),
                Method::WalletMultisigDescriptor => schema_for!(response::WalletMultisigDescriptor),
                Method::SignerRegisterMultisig => schema_for!(response::Empty),
                Method::SignerXpub => schema_for!(response::SignerXpub),
                Method::SignerSign => schema_for!(response::Pset),
                Method::WalletBroadcast => schema_for!(response::WalletBroadcast),
                Method::WalletDetails => schema_for!(response::WalletDetails),
                Method::WalletCombine => schema_for!(response::WalletCombine),
                Method::WalletPsetDetails => schema_for!(response::WalletPsetDetails),
                Method::WalletUtxos => schema_for!(response::WalletUtxos),
                Method::WalletTxs => schema_for!(response::WalletTxs),
                Method::WalletSetTxMemo => schema_for!(response::Empty),
                Method::WalletSetAddrMemo => schema_for!(response::Empty),
                Method::WalletIssue => schema_for!(response::Pset),
                Method::WalletReissue => schema_for!(response::Pset),
                Method::AssetContract => schema_for!(response::AssetContract),
                Method::AssetDetails => schema_for!(response::AssetDetails),
                Method::AssetList => schema_for!(response::AssetList),
                Method::AssetInsert => schema_for!(response::Empty),
                Method::AssetRemove => schema_for!(request::Empty),
                Method::Scan => schema_for!(response::Empty),
                Method::Stop => schema_for!(request::Empty),
                Method::SignerJadeId => schema_for!(response::JadeId),
                Method::AssetPublish => schema_for!(response::AssetPublish),
            },
        })
    }
}

impl FromStr for Method {
    type Err = MethodNotExist;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "schema" => Method::Schema,
            "signer_generate" => Method::SignerGenerate,
            "version" => Method::Version,
            "wallet_load" => Method::WalletLoad,
            "wallet_unload" => Method::WalletUnload,
            "wallet_list" => Method::WalletList,
            "signer_load_software" => Method::SignerLoadSoftware,
            "signer_load_jade" => Method::SignerLoadJade,
            "signer_load_external" => Method::SignerLoadExternal,
            "signer_details" => Method::SignerDetails,
            "signer_unload" => Method::SignerUnload,
            "signer_list" => Method::SignerList,
            "wallet_address" => Method::WalletAddress,
            "wallet_balance" => Method::WalletBalance,
            "wallet_send_many" => Method::WalletSendMany,
            "signer_singlesig_descriptor" => Method::SignerSinglesigDescriptor,
            "wallet_multisig_descriptor" => Method::WalletMultisigDescriptor,
            "signer_register_multisig" => Method::SignerRegisterMultisig,
            "signer_xpub" => Method::SignerXpub,
            "signer_sign" => Method::SignerSign,
            "wallet_broadcast" => Method::WalletBroadcast,
            "wallet_details" => Method::WalletDetails,
            "wallet_combine" => Method::WalletCombine,
            "wallet_pset_details" => Method::WalletPsetDetails,
            "wallet_utxos" => Method::WalletUtxos,
            "wallet_txs" => Method::WalletTxs,
            "wallet_set_tx_memo" => Method::WalletSetTxMemo,
            "wallet_set_addr_memo" => Method::WalletSetAddrMemo,
            "wallet_issue" => Method::WalletIssue,
            "wallet_reissue" => Method::WalletReissue,
            "asset_contract" => Method::AssetContract,
            "asset_details" => Method::AssetDetails,
            "asset_list" => Method::AssetList,
            "asset_insert" => Method::AssetInsert,
            "asset_remove" => Method::AssetRemove,
            "signer_jade_id" => Method::SignerJadeId,
            "asset_publish" => Method::AssetPublish,
            "scan" => Method::Scan,
            "stop" => Method::Stop,
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
            Method::SignerGenerate => "signer_generate",
            Method::Version => "version",
            Method::WalletLoad => "wallet_load",
            Method::WalletUnload => "wallet_unload",
            Method::WalletList => "wallet_list",
            Method::SignerLoadSoftware => "signer_load_software",
            Method::SignerLoadJade => "signer_load_jade",
            Method::SignerLoadExternal => "signer_load_external",
            Method::SignerDetails => "signer_details",
            Method::SignerUnload => "signer_unload",
            Method::SignerList => "signer_list",
            Method::WalletAddress => "wallet_address",
            Method::WalletBalance => "wallet_balance",
            Method::WalletSendMany => "wallet_send_many",
            Method::SignerSinglesigDescriptor => "signer_singlesig_descriptor",
            Method::WalletMultisigDescriptor => "wallet_multisig_descriptor",
            Method::SignerRegisterMultisig => "signer_register_multisig",
            Method::SignerXpub => "signer_xpub",
            Method::SignerSign => "signer_sign",
            Method::WalletBroadcast => "wallet_broadcast",
            Method::WalletDetails => "wallet_details",
            Method::WalletCombine => "wallet_combine",
            Method::WalletPsetDetails => "wallet_pset_details",
            Method::WalletUtxos => "wallet_utxos",
            Method::WalletTxs => "wallet_txs",
            Method::WalletSetTxMemo => "wallet_set_tx_memo",
            Method::WalletSetAddrMemo => "wallet_set_addr_memo",
            Method::WalletIssue => "wallet_issue",
            Method::WalletReissue => "wallet_reissue",
            Method::AssetContract => "asset_contract",
            Method::AssetDetails => "asset_details",
            Method::AssetList => "asset_list",
            Method::AssetInsert => "asset_insert",
            Method::AssetRemove => "asset_remove",
            Method::Scan => "scan",
            Method::Stop => "stop",
            Method::SignerJadeId => "signer_jade_id",
            Method::AssetPublish => "asset_publish",
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
