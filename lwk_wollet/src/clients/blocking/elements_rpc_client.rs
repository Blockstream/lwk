use crate::{clients::try_unblind, Chain, ElementsNetwork, Error, WalletTxOut, WolletDescriptor};

use std::collections::HashMap;

use bitcoincore_rpc::{Auth, Client, RpcApi};
use elements::{
    encode::deserialize, hex::FromHex, Address, OutPoint, Script, Transaction, TxOut, Txid,
};

/// A client to issue RPCs to a Elements node
pub struct ElementsRpcClient {
    inner: Client,
    #[allow(unused)]
    network: ElementsNetwork,
    #[allow(unused)]
    auth: Auth,
    #[allow(unused)]
    url: String,
}

impl ElementsRpcClient {
    /// Create a new Elements RPC client
    pub fn new(network: ElementsNetwork, url: &str, auth: Auth) -> Result<Self, Error> {
        let inner = Client::new(url, auth.clone())?;
        Ok(Self {
            inner,
            network,
            auth,
            url: url.to_string(),
        })
    }

    /// Create a new Elements RPC client from credentials
    pub fn new_from_credentials(
        network: ElementsNetwork,
        url: &str,
        user: &str,
        pass: &str,
    ) -> Result<Self, Error> {
        let auth = Auth::UserPass(user.to_string(), pass.to_string());
        Self::new(network, url, auth)
    }

    /// Get the blockchain height
    pub fn height(&self) -> Result<u64, Error> {
        self.inner
            .call::<serde_json::Value>("getblockcount", &[])?
            .as_u64()
            .ok_or_else(|| Error::ElementsRpcUnexpectedReturn("getblockcount".into()))
    }

    fn get_txout(&self, outpoint: &OutPoint, height: u32) -> Result<TxOut, Error> {
        let blockhash = self
            .inner
            .call::<serde_json::Value>("getblockhash", &[height.into()])?;

        let method = "getrawtransaction";
        let txid = outpoint.txid.to_string();
        let r = self
            .inner
            .call::<serde_json::Value>(method, &[txid.into(), false.into(), blockhash])?;
        let hex = r
            .as_str()
            .ok_or_else(|| Error::ElementsRpcUnexpectedReturn(method.into()))?;
        let bytes = Vec::<u8>::from_hex(hex)
            .map_err(|_| Error::ElementsRpcUnexpectedReturn(method.into()))?;
        let tx: Transaction = deserialize(&bytes[..])
            .map_err(|_| Error::ElementsRpcUnexpectedReturn(method.into()))?;
        let txout = tx
            .output
            .get(outpoint.vout as usize)
            .ok_or_else(|| Error::ElementsRpcUnexpectedReturn(method.into()))?
            .clone();
        Ok(txout)
    }

    /// Get the confirmed utxos for a descriptor
    pub fn confirmed_utxos(
        &self,
        desc: &WolletDescriptor,
        range: u32,
    ) -> Result<Vec<WalletTxOut>, Error> {
        let scanobjects = desc
            .single_bitcoin_descriptors()?
            .iter()
            .map(|d| ScanObject {
                desc: d.to_string(),
                range,
            })
            .collect::<Vec<_>>();
        let scanobjects = serde_json::to_value(scanobjects)?;
        let r: ScanResult = self
            .inner
            .call("scantxoutset", &["start".into(), scanobjects])?;
        let mut utxos = vec![];

        // TODO: make this more efficient
        let params = self.network.address_params();
        let mut spk_map = HashMap::new();
        let mut address_map = HashMap::new();
        for i in 0..range {
            let address = desc.address(i, params)?;
            let spk_ext = address.script_pubkey();
            address_map.insert(spk_ext.clone(), address);
            let change = desc.change(i, params)?;
            let spk_int = change.script_pubkey();
            address_map.insert(spk_int.clone(), change);
            spk_map.insert(spk_ext, (Chain::External, i));
            spk_map.insert(spk_int, (Chain::Internal, i));
        }

        for u in r.unspents {
            let outpoint = OutPoint::new(u.txid, u.vout);
            let (ext_int, wildcard_index) = *spk_map
                .get(&u.script_pubkey)
                .ok_or_else(|| Error::ElementsRpcUnexpectedReturn("scantxoutset".into()))?;
            let txout = self.get_txout(&outpoint, u.height)?;
            let unblinded = try_unblind(&txout, desc)?;
            let address =
                Address::from_script(&u.script_pubkey, None, self.network.address_params())
                    .expect("used descriptors have addresses"); // TODO: get blinding key
            utxos.push(WalletTxOut {
                outpoint,
                script_pubkey: u.script_pubkey,
                height: Some(u.height),
                unblinded,
                wildcard_index,
                ext_int,
                is_spent: false,
                address,
            })
        }
        Ok(utxos)
    }
}

#[derive(serde::Serialize)]
struct ScanObject {
    desc: String,
    range: u32,
}

#[derive(serde::Deserialize)]
struct Unspent {
    txid: Txid,
    vout: u32,
    height: u32, // scantxoutset does not return mempool txs
    #[serde(rename = "scriptPubKey")]
    script_pubkey: Script,
}

#[derive(serde::Deserialize)]
struct ScanResult {
    unspents: Vec<Unspent>,
}
