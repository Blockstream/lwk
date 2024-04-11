use std::net::SocketAddr;

use lwk_jade::TIMEOUT;
use lwk_wollet::UnvalidatedRecipient;
use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::value::to_raw_value;
use serde_json::value::RawValue;
use serde_json::Value;

use crate::error::Error;
use crate::method::Method;
use crate::{request, response};

pub struct Client {
    client: jsonrpc::Client,
}

impl Client {
    pub fn new(addr: SocketAddr) -> Result<Self, Error> {
        let url = format!("http://{addr}");
        let transport = jsonrpc::simple_http::Builder::new()
            .timeout(TIMEOUT)
            // todo: auth
            .url(&url)?
            .build();
        let client = jsonrpc::Client::with_transport(transport);
        Ok(Self { client })
    }

    pub(crate) fn make_request<Req, Res>(
        &self,
        method: Method,
        req: Option<Req>,
    ) -> std::result::Result<Res, Error>
    where
        Req: Serialize,
        Res: DeserializeOwned,
    {
        let params = req.map(|req| to_raw_value(&req)).transpose()?;
        let method = method.to_string();
        let request = self.client.build_request(&method, params.as_deref());
        tracing::trace!("---> {}", serde_json::to_string(&request)?);
        let response = self.client.send_request(request)?;
        tracing::trace!("<--- {}", serde_json::to_string(&response)?);
        match response.result.as_ref() {
            Some(result) => Ok(serde_json::from_str(result.get())?),
            None => match response.error {
                Some(rpc_err) => Err(Error::RpcError(rpc_err)),
                None => Err(Error::NeitherResultNorErrorSet),
            },
        }
    }

    pub fn version(&self) -> Result<response::Version, Error> {
        self.make_request(Method::Version, None::<Box<RawValue>>)
    }

    pub fn signer_generate(&self) -> Result<response::SignerGenerate, Error> {
        self.make_request(Method::SignerGenerate, None::<Box<RawValue>>)
    }

    pub fn signer_load_software(
        &self,
        name: String,
        mnemonic: String,
    ) -> Result<response::Signer, Error> {
        let req = request::SignerLoadSoftware { name, mnemonic };
        self.make_request(Method::SignerLoadSoftware, Some(req))
    }

    pub fn signer_load_jade(
        &self,
        name: String,
        id: String,
        emulator: Option<SocketAddr>,
    ) -> Result<response::Signer, Error> {
        let req = request::SignerLoadJade { name, id, emulator };
        self.make_request(Method::SignerLoadJade, Some(req))
    }

    pub fn signer_load_external(
        &self,
        name: String,
        fingerprint: String,
    ) -> Result<response::Signer, Error> {
        let req = request::SignerLoadExternal { name, fingerprint };
        self.make_request(Method::SignerLoadExternal, Some(req))
    }

    pub fn list_wallets(&self) -> Result<response::ListWallets, Error> {
        self.make_request(Method::ListWallets, None::<Box<RawValue>>)
    }

    pub fn load_wallet(&self, descriptor: String, name: String) -> Result<response::Wallet, Error> {
        let req = request::LoadWallet { descriptor, name };
        self.make_request(Method::LoadWallet, Some(req))
    }

    pub fn wallet_unload(&self, name: String) -> Result<response::WalletUnload, Error> {
        let req = request::WalletUnload { name };
        self.make_request(Method::WalletUnload, Some(req))
    }

    pub fn signer_unload(&self, name: String) -> Result<response::SignerUnload, Error> {
        let req = request::SignerUnload { name };
        self.make_request(Method::SignerUnload, Some(req))
    }

    pub fn signer_list(&self) -> Result<response::SignerList, Error> {
        self.make_request(Method::SignerList, None::<Box<RawValue>>)
    }

    pub fn balance(&self, name: String, with_tickers: bool) -> Result<response::Balance, Error> {
        let req = request::Balance { name, with_tickers };
        self.make_request(Method::Balance, Some(req))
    }

    pub fn address(
        &self,
        name: String,
        index: Option<u32>,
        signer: Option<String>,
    ) -> Result<response::Address, Error> {
        let req = request::Address {
            name,
            index,
            signer,
        };
        self.make_request(Method::Address, Some(req))
    }

    pub fn send_many(
        &self,
        name: String,
        addressees: Vec<UnvalidatedRecipient>,
        fee_rate: Option<f32>,
    ) -> Result<response::Pset, Error> {
        let req = request::Send {
            addressees: addressees.into_iter().map(unvalidate_addressee).collect(),
            fee_rate,
            name,
        };
        self.make_request(Method::SendMany, Some(req))
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
        self.make_request(Method::SinglesigDescriptor, Some(req))
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
        self.make_request(Method::MultisigDescriptor, Some(req))
    }

    pub fn xpub(&self, name: String, xpub_kind: String) -> Result<response::Xpub, Error> {
        let req = request::Xpub { name, xpub_kind };
        self.make_request(Method::Xpub, Some(req))
    }

    pub fn register_multisig(
        &self,
        name: String,
        wallet: String,
    ) -> Result<response::Empty, Error> {
        let req = request::RegisterMultisig { name, wallet };
        self.make_request(Method::RegisterMultisig, Some(req))
    }

    pub fn sign(&self, name: String, pset: String) -> Result<response::Pset, Error> {
        let req = request::Sign { name, pset };
        self.make_request(Method::Sign, Some(req))
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
        self.make_request(Method::Broadcast, Some(req))
    }

    pub fn wallet_details(&self, name: String) -> Result<response::WalletDetails, Error> {
        let req = request::WalletDetails { name };
        self.make_request(Method::WalletDetails, Some(req))
    }

    pub fn signer_details(&self, name: String) -> Result<response::SignerDetails, Error> {
        let req = request::SignerDetails { name };
        self.make_request(Method::SignerDetails, Some(req))
    }

    pub fn wallet_combine(
        &self,
        name: String,
        pset: Vec<String>,
    ) -> Result<response::WalletCombine, Error> {
        let req = request::WalletCombine { name, pset };
        self.make_request(Method::WalletCombine, Some(req))
    }

    pub fn wallet_pset_details(
        &self,
        name: String,
        pset: String,
        with_tickers: bool,
    ) -> Result<response::WalletPsetDetails, Error> {
        let req = request::WalletPsetDetails {
            name,
            pset,
            with_tickers,
        };
        self.make_request(Method::WalletPsetDetails, Some(req))
    }

    pub fn wallet_utxos(&self, name: String) -> Result<response::WalletUtxos, Error> {
        let req = request::WalletUtxos { name };
        self.make_request(Method::WalletUtxos, Some(req))
    }

    pub fn wallet_txs(
        &self,
        name: String,
        with_tickers: bool,
    ) -> Result<response::WalletTxs, Error> {
        let req = request::WalletTxs { name, with_tickers };
        self.make_request(Method::WalletTxs, Some(req))
    }

    pub fn wallet_set_tx_memo(
        &self,
        name: String,
        txid: String,
        memo: String,
    ) -> Result<response::Empty, Error> {
        let req = request::WalletSetTxMemo { name, txid, memo };
        self.make_request(Method::WalletSetTxMemo, Some(req))
    }

    pub fn wallet_set_addr_memo(
        &self,
        name: String,
        address: String,
        memo: String,
    ) -> Result<response::Empty, Error> {
        let req = request::WalletSetAddrMemo {
            name,
            address,
            memo,
        };
        self.make_request(Method::WalletSetAddrMemo, Some(req))
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
        self.make_request(Method::Issue, Some(req))
    }

    pub fn reissue(
        &self,
        name: String,
        asset: String,
        satoshi_asset: u64,
        address_asset: Option<String>,
        fee_rate: Option<f32>,
    ) -> Result<response::Pset, Error> {
        let req = request::Reissue {
            name,
            asset,
            satoshi_asset,
            address_asset,
            fee_rate,
        };
        self.make_request(Method::Reissue, Some(req))
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
        self.make_request(Method::Contract, Some(req))
    }

    pub fn asset_details(&self, asset_id: String) -> Result<response::AssetDetails, Error> {
        let req = request::AssetDetails { asset_id };
        self.make_request(Method::AssetDetails, Some(req))
    }

    pub fn list_assets(&self) -> Result<response::ListAssets, Error> {
        self.make_request(Method::ListAssets, None::<Box<RawValue>>)
    }

    pub fn asset_insert(
        &self,
        asset_id: String,
        issuance_tx: String,
        contract: String,
    ) -> Result<response::Empty, Error> {
        let req = request::AssetInsert {
            asset_id,
            issuance_tx,
            contract,
        };
        self.make_request(Method::AssetInsert, Some(req))
    }

    pub fn asset_remove(&self, asset_id: String) -> Result<response::Empty, Error> {
        let req = request::AssetRemove { asset_id };
        self.make_request(Method::AssetRemove, Some(req))
    }

    pub fn asset_publish(&self, asset_id: String) -> Result<response::AssetPublish, Error> {
        let req = request::AssetPublish { asset_id };
        self.make_request(Method::AssetPublish, Some(req))
    }

    pub fn schema(&self, arg: Method, direction: request::Direction) -> Result<Value, Error> {
        let req = request::Schema {
            method: arg.to_string(),
            direction,
        };
        self.make_request(Method::Schema, Some(req))
    }

    pub fn signer_jade_id(&self, emulator: Option<SocketAddr>) -> Result<Value, Error> {
        let req = request::SignerJadeId { emulator };
        self.make_request(Method::SignerJadeId, Some(req))
    }

    pub fn scan(&self) -> Result<Value, Error> {
        self.make_request(Method::Scan, None::<Box<RawValue>>)
    }

    pub fn stop(&self) -> Result<Value, Error> {
        // TODO discriminate only stop error
        let _: Result<Value, Error> = self.make_request(Method::Stop, None::<Box<RawValue>>);
        Ok(Value::Null)
    }
}

fn unvalidate_addressee(a: lwk_wollet::UnvalidatedRecipient) -> request::UnvalidatedAddressee {
    request::UnvalidatedAddressee {
        satoshi: a.satoshi,
        address: a.address,
        asset: a.asset,
    }
}
