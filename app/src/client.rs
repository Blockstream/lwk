use std::net::SocketAddr;

use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::value::to_raw_value;
use serde_json::value::RawValue;
use serde_json::Value;
use wollet::UnvalidatedAddressee;

use crate::error::Error;
use crate::{request, response};

pub struct Client {
    client: jsonrpc::Client,
}

impl Client {
    pub fn new(addr: SocketAddr) -> Result<Self, Error> {
        let url = addr.to_string();
        let client = jsonrpc::Client::simple_http(&url, None, None)?; // todo: auth
        Ok(Self { client })
    }

    fn make_request<Req, Res>(
        &self,
        method: &str,
        req: Option<Req>,
    ) -> std::result::Result<Res, Error>
    where
        Req: Serialize,
        Res: DeserializeOwned,
    {
        let params = req.map(|req| to_raw_value(&req)).transpose()?;
        let request = self.client.build_request(method, params.as_deref());
        tracing::trace!("---> {}", serde_json::to_string(&request).unwrap());
        let response = self.client.send_request(request)?;
        tracing::trace!("<--- {}", serde_json::to_string(&response).unwrap());
        match response.result.as_ref() {
            Some(result) => Ok(serde_json::from_str(result.get()).unwrap()),
            None => match response.error {
                Some(rpc_err) => Err(Error::RpcError(rpc_err)),
                None => Err(Error::NeitherResultNorErrorSet),
            },
        }
    }

    pub fn version(&self) -> Result<response::Version, Error> {
        self.make_request("version", None::<Box<RawValue>>)
    }

    pub fn generate_signer(&self) -> Result<response::GenerateSigner, Error> {
        self.make_request("generate_signer", None::<Box<RawValue>>)
    }

    pub fn load_signer(
        &self,
        name: String,
        kind: String,
        mnemonic: Option<String>,
        fingerprint: Option<String>,
    ) -> Result<response::Signer, Error> {
        let req = request::LoadSigner {
            name,
            kind,
            mnemonic,
            fingerprint,
        };
        self.make_request("load_signer", Some(req))
    }

    pub fn list_wallets(&self) -> Result<response::ListWallets, Error> {
        self.make_request("list_wallets", None::<Box<RawValue>>)
    }

    pub fn load_wallet(&self, descriptor: String, name: String) -> Result<response::Wallet, Error> {
        let req = request::LoadWallet { descriptor, name };
        self.make_request("load_wallet", Some(req))
    }

    pub fn unload_wallet(&self, name: String) -> Result<response::UnloadWallet, Error> {
        let req = request::UnloadWallet { name };
        self.make_request("unload_wallet", Some(req))
    }

    pub fn unload_signer(&self, name: String) -> Result<response::UnloadSigner, Error> {
        let req = request::UnloadSigner { name };
        self.make_request("unload_signer", Some(req))
    }

    pub fn list_signers(&self) -> Result<response::ListSigners, Error> {
        self.make_request("list_signers", None::<Box<RawValue>>)
    }

    pub fn balance(&self, name: String) -> Result<response::Balance, Error> {
        let req = request::Balance { name };
        self.make_request("balance", Some(req))
    }

    pub fn address(&self, name: String, index: Option<u32>) -> Result<response::Address, Error> {
        let req = request::Address { name, index };
        self.make_request("address", Some(req))
    }

    pub fn send_many(
        &self,
        name: String,
        addressees: Vec<UnvalidatedAddressee>,
        fee_rate: Option<f32>,
    ) -> Result<response::Pset, Error> {
        let req = request::Send {
            addressees: addressees.into_iter().map(unvalidate_addressee).collect(),
            fee_rate,
            name,
        };
        self.make_request("send_many", Some(req))
    }

    pub fn singlesig_descriptor(
        &self,
        name: String,
        descriptor_blinding_key: String,
        singlesig_kind: String,
    ) -> Result<response::SinglesigDescriptor, Error> {
        let req = request::SinglesigDescriptor {
            name,
            descriptor_blinding_key,
            singlesig_kind,
        };
        self.make_request("singlesig_descriptor", Some(req))
    }

    pub fn multisig_descriptor(
        &self,
        descriptor_blinding_key: String,
        multisig_kind: String,
        threshold: u32,
        keyorigin_xpubs: Vec<String>,
    ) -> Result<response::MultisigDescriptor, Error> {
        let req = request::MultisigDescriptor {
            descriptor_blinding_key,
            multisig_kind,
            threshold,
            keyorigin_xpubs,
        };
        self.make_request("multisig_descriptor", Some(req))
    }

    pub fn xpub(&self, name: String, xpub_kind: String) -> Result<response::Xpub, Error> {
        let req = request::Xpub { name, xpub_kind };
        self.make_request("xpub", Some(req))
    }

    pub fn sign(&self, name: String, pset: String) -> Result<response::Pset, Error> {
        let req = request::Sign { name, pset };
        self.make_request("sign", Some(req))
    }

    pub fn broadcast(
        &self,
        name: String,
        dry_run: bool,
        pset: String,
    ) -> Result<response::Broadcast, Error> {
        let req = request::Broadcast {
            name,
            dry_run,
            pset,
        };
        self.make_request("broadcast", Some(req))
    }

    pub fn wallet_details(&self, name: String) -> Result<response::WalletDetails, Error> {
        let req = request::WalletDetails { name };
        self.make_request("wallet_details", Some(req))
    }

    pub fn wallet_combine(
        &self,
        name: String,
        pset: Vec<String>,
    ) -> Result<response::WalletCombine, Error> {
        let req = request::WalletCombine { name, pset };
        self.make_request("wallet_combine", Some(req))
    }

    pub fn wallet_pset_details(
        &self,
        name: String,
        pset: String,
    ) -> Result<response::WalletPsetDetails, Error> {
        let req = request::WalletPsetDetails { name, pset };
        self.make_request("wallet_pset_details", Some(req))
    }

    #[allow(clippy::too_many_arguments)]
    pub fn issue(
        &self,
        name: String,
        satoshi_asset: u64,
        address_asset: Option<String>,
        satoshi_token: u64,
        address_token: Option<String>,
        contract: Option<String>,
        fee_rate: Option<f32>,
    ) -> Result<response::Pset, Error> {
        let req = request::Issue {
            name,
            satoshi_asset,
            address_asset,
            satoshi_token,
            address_token,
            contract,
            fee_rate,
        };
        self.make_request("issue", Some(req))
    }

    pub fn contract(
        &self,
        domain: String,
        issuer_pubkey: String,
        name: String,
        precision: u8,
        ticker: String,
        version: u8,
    ) -> Result<response::Contract, Error> {
        let req = request::Contract {
            domain,
            issuer_pubkey,
            name,
            precision,
            ticker,
            version,
        };
        self.make_request("contract", Some(req))
    }

    pub fn stop(&self) -> Result<Value, Error> {
        // TODO discriminate only stop error
        let _: Result<Value, Error> = self.make_request("stop", None::<Box<RawValue>>);
        Ok(Value::Null)
    }
}

fn unvalidate_addressee(a: wollet::UnvalidatedAddressee) -> request::UnvalidatedAddressee {
    request::UnvalidatedAddressee {
        satoshi: a.satoshi,
        address: a.address,
        asset: a.asset,
    }
}
