use std::str::FromStr;

#[derive(Debug, thiserror::Error)]
#[error("The rpc method '{name}' does not exist")]
pub struct MethodNotExist {
    name: String,
}
pub(crate) enum Method {
    GenerateSigner,
    Version,
    LoadWallet,
    UnloadWallet,
    ListWallets,
    LoadSigner,
    UnloadSigner,
    ListSigners,
    Address,
    Balance,
    SendMany,
    SinglesigDescriptor,
    MultisigDescriptor,
    Xpub,
    Sign,
    Broadcast,
    WalletDetails,
    WalletCombine,
    WalletPsetDetails,
    Issue,
    Contract,
    Stop,
}

impl FromStr for Method {
    type Err = MethodNotExist;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "generate_signer" => Method::GenerateSigner,
            "version" => Method::Version,
            "load_wallet" => Method::LoadWallet,
            "unload_wallet" => Method::UnloadWallet,
            "list_wallets" => Method::ListWallets,
            "load_signer" => Method::LoadSigner,
            "unload_signer" => Method::UnloadSigner,
            "list_signers" => Method::ListSigners,
            "address" => Method::Address,
            "balance" => Method::Balance,
            "send_many" => Method::SendMany,
            "singlesig_descriptor" => Method::SinglesigDescriptor,
            "multisig_descriptor" => Method::MultisigDescriptor,
            "xpub" => Method::Xpub,
            "sign" => Method::Sign,
            "broadcast" => Method::Broadcast,
            "wallet_details" => Method::WalletDetails,
            "wallet_combine" => Method::WalletCombine,
            "wallet_pset_details" => Method::WalletPsetDetails,
            "issue" => Method::Issue,
            "contract" => Method::Contract,
            "stop" => Method::Stop,
            _ => {
                return Err(MethodNotExist {
                    name: s.to_string(),
                })
            }
        })
    }
}
