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
use crate::reqwest_transport::ReqwestHttpTransport;
use crate::{request, response};

pub struct Client {
    client: jsonrpc::Client,
}

impl Client {
    pub fn new(addr: SocketAddr) -> Result<Self, Error> {
        let url = format!("http://{addr}");
        let transport = ReqwestHttpTransport::new(url, TIMEOUT);
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
        log::trace!("---> {}", serde_json::to_string(&request)?);
        let response = self.client.send_request(request)?;
        log::trace!("<--- {}", serde_json::to_string(&response)?);
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
        persist: bool,
    ) -> Result<response::Signer, Error> {
        let req = request::SignerLoadSoftware {
            name,
            mnemonic,
            persist,
        };
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

    pub fn wallet_list(&self) -> Result<response::WalletList, Error> {
        self.make_request(Method::WalletList, None::<Box<RawValue>>)
    }

    pub fn wallet_load(&self, descriptor: String, name: String) -> Result<response::Wallet, Error> {
        let req = request::WalletLoad { descriptor, name };
        self.make_request(Method::WalletLoad, Some(req))
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

    pub fn wallet_balance(
        &self,
        name: String,
        with_tickers: bool,
    ) -> Result<response::WalletBalance, Error> {
        let req = request::WalletBalance { name, with_tickers };
        self.make_request(Method::WalletBalance, Some(req))
    }

    pub fn wallet_address(
        &self,
        name: String,
        index: Option<u32>,
        signer: Option<String>,
        with_text_qr: bool,
        with_uri_qr: Option<u8>,
    ) -> Result<response::WalletAddress, Error> {
        let req = request::WalletAddress {
            name,
            index,
            signer,
            with_text_qr,
            with_uri_qr,
        };
        self.make_request(Method::WalletAddress, Some(req))
    }

    pub fn wallet_send_many(
        &self,
        name: String,
        addressees: Vec<UnvalidatedRecipient>,
        fee_rate: Option<f32>,
    ) -> Result<response::Pset, Error> {
        let req = request::WalletSendMany {
            addressees: addressees.into_iter().map(unvalidate_addressee).collect(),
            fee_rate,
            name,
        };
        self.make_request(Method::WalletSendMany, Some(req))
    }

    pub fn wallet_drain(
        &self,
        name: String,
        address: String,
        fee_rate: Option<f32>,
    ) -> Result<response::Pset, Error> {
        let req = request::WalletDrain {
            address,
            fee_rate,
            name,
        };
        self.make_request(Method::WalletDrain, Some(req))
    }

    pub fn signer_singlesig_descriptor(
        &self,
        name: String,
        descriptor_blinding_key: String,
        singlesig_kind: String,
    ) -> Result<response::SignerSinglesigDescriptor, Error> {
        let req = request::SignerSinglesigDescriptor {
            name,
            descriptor_blinding_key,
            singlesig_kind,
        };
        self.make_request(Method::SignerSinglesigDescriptor, Some(req))
    }

    pub fn wallet_multisig_descriptor(
        &self,
        descriptor_blinding_key: String,
        multisig_kind: String,
        threshold: u32,
        keyorigin_xpubs: Vec<String>,
    ) -> Result<response::WalletMultisigDescriptor, Error> {
        let req = request::WalletMultisigDescriptor {
            descriptor_blinding_key,
            multisig_kind,
            threshold,
            keyorigin_xpubs,
        };
        self.make_request(Method::WalletMultisigDescriptor, Some(req))
    }

    pub fn signer_xpub(
        &self,
        name: String,
        xpub_kind: String,
    ) -> Result<response::SignerXpub, Error> {
        let req = request::SignerXpub { name, xpub_kind };
        self.make_request(Method::SignerXpub, Some(req))
    }

    pub fn signer_register_multisig(
        &self,
        name: String,
        wallet: String,
    ) -> Result<response::Empty, Error> {
        let req = request::SignerRegisterMultisig { name, wallet };
        self.make_request(Method::SignerRegisterMultisig, Some(req))
    }

    pub fn signer_sign(&self, name: String, pset: String) -> Result<response::Pset, Error> {
        let req = request::SignerSign { name, pset };
        self.make_request(Method::SignerSign, Some(req))
    }

    pub fn wallet_broadcast(
        &self,
        name: String,
        dry_run: bool,
        pset: String,
    ) -> Result<response::WalletBroadcast, Error> {
        let req = request::WalletBroadcast {
            name,
            dry_run,
            pset,
        };
        self.make_request(Method::WalletBroadcast, Some(req))
    }

    pub fn wallet_details(&self, name: String) -> Result<response::WalletDetails, Error> {
        let req = request::WalletDetails { name };
        self.make_request(Method::WalletDetails, Some(req))
    }

    pub fn signer_details(&self, name: String) -> Result<response::SignerDetails, Error> {
        let req = request::SignerDetails { name };
        self.make_request(Method::SignerDetails, Some(req))
    }

    pub fn signer_derive_bip85(
        &self,
        name: String,
        index: u32,
        word_count: u32,
    ) -> Result<response::SignerDeriveBip85, Error> {
        let req = request::SignerDeriveBip85 {
            name,
            index,
            word_count,
        };
        self.make_request(Method::SignerDeriveBip85, Some(req))
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

    pub fn wallet_tx(
        &self,
        name: String,
        txid: String,
        fetch: bool,
    ) -> Result<response::WalletTx, Error> {
        let req = request::WalletTx {
            name,
            txid,
            fetch,
        };
        self.make_request(Method::WalletTx, Some(req))
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

    pub fn liquidex_make(
        &self,
        name: String,
        txid: String,
        vout: u32,
        asset: String,
        satoshi: u64,
    ) -> Result<response::Pset, Error> {
        let req = request::LiquidexMake {
            name,
            txid,
            vout,
            asset,
            satoshi,
        };
        self.make_request(Method::LiquidexMake, Some(req))
    }

    pub fn liquidex_take(&self, name: String, proposal: String) -> Result<response::Pset, Error> {
        let req = request::LiquidexTake { name, proposal };
        self.make_request(Method::LiquidexTake, Some(req))
    }

    pub fn liquidex_to_proposal(&self, pset: String) -> Result<response::LiquidexProposal, Error> {
        let req = request::LiquidexToProposal { pset };
        self.make_request(Method::LiquidexToProposal, Some(req))
    }

    #[allow(clippy::too_many_arguments)]
    pub fn wallet_issue(
        &self,
        name: String,
        satoshi_asset: u64,
        address_asset: Option<String>,
        satoshi_token: u64,
        address_token: Option<String>,
        contract: Option<String>,
        fee_rate: Option<f32>,
    ) -> Result<response::Pset, Error> {
        let req = request::WalletIssue {
            name,
            satoshi_asset,
            address_asset,
            satoshi_token,
            address_token,
            contract,
            fee_rate,
        };
        self.make_request(Method::WalletIssue, Some(req))
    }

    pub fn wallet_reissue(
        &self,
        name: String,
        asset: String,
        satoshi_asset: u64,
        address_asset: Option<String>,
        fee_rate: Option<f32>,
    ) -> Result<response::Pset, Error> {
        let req = request::WalletReissue {
            name,
            asset,
            satoshi_asset,
            address_asset,
            fee_rate,
        };
        self.make_request(Method::WalletReissue, Some(req))
    }

    pub fn wallet_burn(
        &self,
        name: String,
        asset: String,
        satoshi_asset: u64,
        fee_rate: Option<f32>,
    ) -> Result<response::Pset, Error> {
        let req = request::WalletBurn {
            name,
            asset,
            satoshi_asset,
            fee_rate,
        };
        self.make_request(Method::WalletBurn, Some(req))
    }

    pub fn asset_contract(
        &self,
        domain: String,
        issuer_pubkey: String,
        name: String,
        precision: u8,
        ticker: String,
        version: u8,
    ) -> Result<response::AssetContract, Error> {
        let req = request::AssetContract {
            domain,
            issuer_pubkey,
            name,
            precision,
            ticker,
            version,
        };
        self.make_request(Method::AssetContract, Some(req))
    }

    pub fn asset_details(&self, asset_id: String) -> Result<response::AssetDetails, Error> {
        let req = request::AssetDetails { asset_id };
        self.make_request(Method::AssetDetails, Some(req))
    }

    pub fn asset_list(&self) -> Result<response::AssetList, Error> {
        self.make_request(Method::AssetList, None::<Box<RawValue>>)
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

    pub fn asset_from_registry(&self, asset_id: String) -> Result<response::Empty, Error> {
        let req = request::AssetFromRegistry { asset_id };
        self.make_request(Method::AssetFromRegistry, Some(req))
    }

    pub fn asset_publish(&self, asset_id: String) -> Result<response::AssetPublish, Error> {
        let req = request::AssetPublish { asset_id };
        self.make_request(Method::AssetPublish, Some(req))
    }

    pub fn amp2_descriptor(&self, name: String) -> Result<response::Amp2Descriptor, Error> {
        let req = request::Amp2Descriptor { name };
        self.make_request(Method::Amp2Descriptor, Some(req))
    }

    pub fn amp2_register(&self, name: String) -> Result<response::Amp2Register, Error> {
        let req = request::Amp2Register { name };
        self.make_request(Method::Amp2Register, Some(req))
    }

    pub fn amp2_cosign(&self, pset: String) -> Result<response::Amp2Cosign, Error> {
        let req = request::Amp2Cosign { pset };
        self.make_request(Method::Amp2Cosign, Some(req))
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
