use crate::bitcoin::bip32::Fingerprint;
use crate::config::{Config, ElementsNetwork};
use crate::descriptor::Chain;
use crate::elements::confidential::Value;
use crate::elements::encode::{
    deserialize as elements_deserialize, serialize as elements_serialize,
};
use crate::elements::issuance::ContractHash;
use crate::elements::pset::PartiallySignedTransaction;
use crate::elements::secp256k1_zkp::ZERO_TWEAK;
use crate::elements::{AssetId, BlockHash, BlockHeader, OutPoint, Script, Transaction, Txid};
use crate::error::Error;
use crate::hashes::{sha256, Hash};
use crate::model::{AddressResult, IssuanceDetails, WalletTxOut};
use crate::store::{new_store, Store};
use crate::sync::sync;
use crate::util::EC;
use crate::WolletDescriptor;
use common::{pset_balance, pset_issuances, pset_signatures, PsetDetails};
use electrum_client::bitcoin::bip32::ChildNumber;
use electrum_client::ElectrumApi;
use elements_miniscript::psbt::PsbtExt;
use elements_miniscript::{psbt, ForEachKey};
use elements_miniscript::{
    ConfidentialDescriptor, DefiniteDescriptorKey, Descriptor, DescriptorPublicKey,
};
use std::cmp::Ordering;
use std::collections::HashMap;
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::atomic;

/// A watch-only wallet defined by a CT descriptor.
pub struct Wollet {
    pub(crate) config: Config,
    pub(crate) store: Store,
    descriptor: WolletDescriptor,
}

impl Wollet {
    /// Create a new  wallet
    pub fn new(
        network: ElementsNetwork,
        electrum_url: &str,
        tls: bool,
        validate_domain: bool,
        data_root: &str,
        desc: &str,
    ) -> Result<Self, Error> {
        let config = Config::new(network, tls, validate_domain, electrum_url, data_root)?;
        Self::inner_new(config, desc)
    }

    fn inner_new(config: Config, desc: &str) -> Result<Self, Error> {
        let descriptor = WolletDescriptor::from_str(desc)?;

        let wallet_desc = format!("{}{:?}", desc, config);
        let wallet_id = format!("{}", sha256::Hash::hash(wallet_desc.as_bytes()));

        let mut path: PathBuf = config.data_root().into();
        if !path.exists() {
            std::fs::create_dir_all(&path)?;
        }
        path.push(wallet_id);
        let store = new_store(&path, &descriptor)?;

        Ok(Wollet {
            store,
            config,
            descriptor,
        })
    }

    /// Get the network policy asset
    pub fn policy_asset(&self) -> AssetId {
        self.config.policy_asset()
    }

    /// Get the wallet descriptor
    pub fn descriptor(&self) -> &ConfidentialDescriptor<DescriptorPublicKey> {
        self.descriptor.as_ref()
    }

    /// Sync the wallet transactions
    pub fn sync_txs(&mut self) -> Result<(), Error> {
        tracing::trace!("Start sync_txs");
        if let Ok(client) = self.config.electrum_url().build_client() {
            let descriptor = self.descriptor.clone();
            match sync(&client, &mut self.store, &descriptor) {
                Ok(true) => tracing::info!("there are new transactions"),
                Ok(false) => tracing::debug!("there aren't new transactions"),
                Err(e) => tracing::warn!("Error during sync, {:?}", e),
            }
        }
        Ok(())
    }

    /// Sync the blockchain tip
    pub fn sync_tip(&mut self) -> Result<(), Error> {
        if let Ok(client) = self.config.electrum_url().build_client() {
            let header = client.block_headers_subscribe_raw()?;
            let height = header.height as u32;
            let tip_height = self.store.cache.tip.0;
            if height != tip_height {
                let block_header: BlockHeader = elements_deserialize(&header.header)?;
                let hash: BlockHash = block_header.block_hash();
                self.store.cache.tip = (height, hash);
            }
        }
        Ok(())
    }

    /// Get the blockchain tip
    pub fn tip(&self) -> Result<(u32, BlockHash), Error> {
        Ok(self.store.cache.tip)
    }

    /// Get a wallet address
    ///
    /// If Some return the address at the given index,
    /// otherwise the last unused address.
    pub fn address(&self, index: Option<u32>) -> Result<AddressResult, Error> {
        let index = match index {
            Some(i) => i,
            None => self
                .store
                .cache
                .last_unused_external
                .load(atomic::Ordering::Relaxed),
        };

        let address = self
            .descriptor
            .address(index, self.config.address_params())?;
        Ok(AddressResult::new(address, index))
    }

    /// Get a wallet change address
    ///
    /// If a specific descriptor is given for change addresses  it's used to derive this address
    /// Otherwise this is the same as `address()`
    ///
    /// If Some return the address at the given index,
    /// otherwise the last unused address.
    pub fn change(&self, index: Option<u32>) -> Result<AddressResult, Error> {
        let index = match index {
            Some(i) => i,
            None => self
                .store
                .cache
                .last_unused_internal
                .load(atomic::Ordering::Relaxed),
        };

        let address = self
            .descriptor
            .change(index, self.config.address_params())?;
        Ok(AddressResult::new(address, index))
    }

    /// Get the wallet UTXOs
    pub fn utxos(&self) -> Result<Vec<WalletTxOut>, Error> {
        let mut txos = vec![];
        let spent = self.store.spent()?;
        for (tx_id, height) in self.store.cache.heights.iter() {
            let tx = self
                .store
                .cache
                .all_txs
                .get(tx_id)
                .ok_or_else(|| Error::Generic(format!("txos no tx {}", tx_id)))?;
            let tx_txos: Vec<WalletTxOut> = {
                tx.output
                    .clone()
                    .into_iter()
                    .enumerate()
                    .map(|(vout, output)| {
                        (
                            OutPoint {
                                txid: tx.txid(),
                                vout: vout as u32,
                            },
                            output,
                        )
                    })
                    .filter(|(outpoint, _)| !spent.contains(outpoint))
                    .filter_map(|(outpoint, output)| {
                        if let Some(unblinded) = self.store.cache.unblinded.get(&outpoint) {
                            let index = self.index(&output.script_pubkey).ok()?;
                            return Some(WalletTxOut {
                                outpoint,
                                script_pubkey: output.script_pubkey,
                                height: *height,
                                unblinded: *unblinded,
                                wildcard_index: index.1,
                                ext_int: index.0,
                            });
                        }
                        None
                    })
                    .collect()
            };
            txos.extend(tx_txos);
        }
        txos.sort_by(|a, b| b.unblinded.value.cmp(&a.unblinded.value));

        Ok(txos)
    }

    pub(crate) fn balance_from_utxos(
        &self,
        utxos: &[WalletTxOut],
    ) -> Result<HashMap<AssetId, u64>, Error> {
        let mut r = HashMap::new();
        r.entry(self.policy_asset()).or_insert(0);
        for u in utxos.iter() {
            *r.entry(u.unblinded.asset).or_default() += u.unblinded.value;
        }
        Ok(r)
    }

    /// Get the wallet balance
    pub fn balance(&self) -> Result<HashMap<AssetId, u64>, Error> {
        let utxos = self.utxos()?;
        self.balance_from_utxos(&utxos)
    }

    /// Get the wallet transactions with their heights (if confirmed)
    pub fn transactions(&self) -> Result<Vec<(Transaction, Option<u32>)>, Error> {
        let mut txs = vec![];
        let mut my_txids: Vec<(&Txid, &Option<u32>)> = self.store.cache.heights.iter().collect();
        my_txids.sort_by(|a, b| {
            let height_cmp =
                b.1.unwrap_or(std::u32::MAX)
                    .cmp(&a.1.unwrap_or(std::u32::MAX));
            match height_cmp {
                Ordering::Equal => b.0.cmp(a.0),
                h => h,
            }
        });

        for (tx_id, height) in my_txids.iter() {
            let tx = self
                .store
                .cache
                .all_txs
                .get(*tx_id)
                .ok_or_else(|| Error::Generic(format!("list_tx no tx {}", tx_id)))?;

            txs.push((tx.clone(), **height));
        }

        Ok(txs)
    }

    /// Get the wallet (re)issuances
    pub fn issuances(&self) -> Result<Vec<IssuanceDetails>, Error> {
        let mut r = vec![];
        for (tx, _) in self.transactions()? {
            for (vin, txin) in tx.input.iter().enumerate() {
                if txin.has_issuance() {
                    let contract_hash =
                        ContractHash::from_byte_array(txin.asset_issuance.asset_entropy);
                    let entropy =
                        AssetId::generate_asset_entropy(txin.previous_output, contract_hash)
                            .to_byte_array();
                    let (asset, token) = txin.issuance_ids();
                    let is_reissuance = txin.asset_issuance.asset_blinding_nonce != ZERO_TWEAK;
                    // FIXME: attempt to unblind if blinded
                    let asset_amount = match txin.asset_issuance.amount {
                        Value::Explicit(a) => Some(a),
                        _ => None,
                    };
                    let token_amount = match txin.asset_issuance.inflation_keys {
                        Value::Explicit(a) => Some(a),
                        _ => None,
                    };
                    // FIXME: comment if the issuance is blinded
                    r.push(IssuanceDetails {
                        txid: tx.txid(),
                        vin: vin as u32,
                        entropy,
                        asset,
                        token,
                        is_reissuance,
                        asset_amount,
                        token_amount,
                    });
                }
            }
        }
        Ok(r)
    }

    /// Get the issuance details for a certain asset
    ///
    /// This only works if the asset was issued by this wallet
    pub fn issuance(&self, asset: &AssetId) -> Result<IssuanceDetails, Error> {
        self.issuances()?
            .iter()
            .find(|d| &d.asset == asset && !d.is_reissuance)
            .cloned()
            .ok_or_else(|| Error::MissingIssuance)
    }

    /// Get the PSET details with respect to the wallet
    pub fn get_details(&self, pset: &PartiallySignedTransaction) -> Result<PsetDetails, Error> {
        Ok(PsetDetails {
            balance: pset_balance(pset, self.descriptor())?,
            sig_details: pset_signatures(pset),
            issuances: pset_issuances(pset),
        })
    }

    pub(crate) fn index(&self, script_pubkey: &Script) -> Result<(Chain, u32), Error> {
        let (ext_int, index) = self
            .store
            .cache
            .paths
            .get(script_pubkey)
            .ok_or_else(|| Error::ScriptNotMine)?;
        let index = match index {
            ChildNumber::Normal { index } => index,
            ChildNumber::Hardened { index: _ } => {
                return Err(Error::Generic("unexpected hardened derivation".into()));
            }
        };
        Ok((*ext_int, *index))
    }

    // TODO: move to WolletDescriptor::definite_descriptor(index)
    pub(crate) fn definite_descriptor(
        &self,
        script_pubkey: &Script,
    ) -> Result<Descriptor<DefiniteDescriptorKey>, Error> {
        let (ext_int, utxo_index) = self.index(script_pubkey)?;
        self.descriptor.definite_descriptor(ext_int, utxo_index)
    }

    /// Add the PSET details with respect to the wallet
    pub fn add_details(&self, pset: &mut PartiallySignedTransaction) -> Result<(), Error> {
        let pset_clone = pset.clone();
        for (idx, input) in pset_clone.inputs().iter().enumerate() {
            if let Some(txout) = input.witness_utxo.as_ref() {
                match self.definite_descriptor(&txout.script_pubkey) {
                    Ok(desc) => {
                        pset.update_input_with_descriptor(idx, &desc)?;
                    }
                    Err(Error::ScriptNotMine) => (),
                    Err(e) => return Err(e),
                }
            }
        }

        for (idx, output) in pset_clone.outputs().iter().enumerate() {
            match self.definite_descriptor(&output.script_pubkey) {
                Ok(desc) => {
                    pset.update_output_with_descriptor(idx, &desc)?;
                }
                Err(Error::ScriptNotMine) => (),
                Err(e) => return Err(e),
            }
        }

        // Set PSET xpub origin
        self.descriptor().descriptor.for_each_key(|k| {
            if let DescriptorPublicKey::XPub(x) = k {
                if let Some(origin) = &x.origin {
                    pset.global.xpub.insert(x.xkey, origin.clone());
                }
            }
            true
        });

        Ok(())
    }

    /// Get the signers' fingerprints involved in this descriptor
    pub fn signers(&self) -> Vec<Fingerprint> {
        let mut signers = vec![];
        self.descriptor().descriptor.for_each_key(|k| {
            // xpub without key origin and single pubkey unexpectedly return a master fingerprint,
            // see tests below for the actual behaviour.
            // This should not be dangerous though, worst case is that we report a signer that
            // cannot sign.
            signers.push(k.master_fingerprint());
            true
        });
        signers
    }

    /// Combine a vector of PSET
    pub fn combine(
        &self,
        psets: &[PartiallySignedTransaction],
    ) -> Result<PartiallySignedTransaction, Error> {
        let mut res = psets.get(0).ok_or_else(|| Error::MissingPset)?.clone();
        for pset in psets.iter().skip(1) {
            res.merge(pset.clone())?;
        }
        Ok(res)
    }

    pub fn finalize(&self, pset: &mut PartiallySignedTransaction) -> Result<Transaction, Error> {
        // genesis_hash is only used for BIP341 (taproot) sighash computation
        psbt::finalize(pset, &EC, BlockHash::all_zeros())?;
        Ok(pset.extract_tx()?)
    }

    pub fn broadcast(&self, tx: &Transaction) -> Result<Txid, Error> {
        let client = self.config.electrum_url().build_client()?;
        let txid = client.transaction_broadcast_raw(&elements_serialize(tx))?;
        Ok(Txid::from_raw_hash(txid.to_raw_hash()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::elements::bitcoin::bip32::{ExtendedPrivKey, ExtendedPubKey};
    use crate::elements::bitcoin::network::constants::Network;
    use crate::elements::AddressParams;
    use elements_miniscript::confidential::bare::tweak_private_key;
    use elements_miniscript::confidential::Key;
    use elements_miniscript::descriptor::checksum::desc_checksum;
    use elements_miniscript::descriptor::DescriptorSecretKey;

    #[test]
    fn test_desc() {
        let xpub = "tpubDD7tXK8KeQ3YY83yWq755fHY2JW8Ha8Q765tknUM5rSvjPcGWfUppDFMpQ1ScziKfW3ZNtZvAD7M3u7bSs7HofjTD3KP3YxPK7X6hwV8Rk2";
        let master_blinding_key =
            "9c8e4f05c7711a98c838be228bcb84924d4570ca53f35fa1c793e58841d47023";
        let checksum = "qw2qy2ml";
        let desc_str = format!(
            "ct(slip77({}),elwpkh({}))#{}",
            master_blinding_key, xpub, checksum
        );
        let desc = ConfidentialDescriptor::<DefiniteDescriptorKey>::from_str(&desc_str).unwrap();
        let addr = desc.address(&EC, &AddressParams::ELEMENTS).unwrap();
        let expected_addr = "el1qqthj9zn320epzlcgd07kktp5ae2xgx82fkm42qqxaqg80l0fszueszj4mdsceqqfpv24x0cmkvd8awux8agrc32m9nj9sp0hk";
        assert_eq!(addr.to_string(), expected_addr.to_string());
    }

    #[test]
    fn test_blinding_private() {
        // Get a confidential address from a "view" descriptor
        let seed = [0u8; 16];
        let xprv = ExtendedPrivKey::new_master(Network::Regtest, &seed).unwrap();
        let xpub = ExtendedPubKey::from_priv(&EC, &xprv);
        let checksum = "h0ej28gv";
        let desc_str = format!("ct({},elwpkh({}))#{}", xprv, xpub, checksum);
        let desc = ConfidentialDescriptor::<DefiniteDescriptorKey>::from_str(&desc_str).unwrap();
        let address = desc.address(&EC, &AddressParams::ELEMENTS).unwrap();
        // and extract the public blinding key
        let pk_from_addr = address.blinding_pubkey.unwrap();

        // Get the public blinding key from the descriptor blinding key
        let key = match desc.key {
            Key::View(DescriptorSecretKey::XPrv(dxk)) => dxk.xkey.to_priv(),
            _ => todo!(),
        };
        let tweaked_key = tweak_private_key(&EC, &address.script_pubkey(), &key.inner);
        let pk_from_view = tweaked_key.public_key(&EC);

        assert_eq!(pk_from_addr, pk_from_view);
    }

    #[test]
    fn test_view_single() {
        let descriptor_blinding_key = "L3jXxwef3fpB7hcrFozcWgHeJCPSAFiZ1Ji2YJMPxceaGvy3PC1q";
        let xpub = "tpubDD7tXK8KeQ3YY83yWq755fHY2JW8Ha8Q765tknUM5rSvjPcGWfUppDFMpQ1ScziKfW3ZNtZvAD7M3u7bSs7HofjTD3KP3YxPK7X6hwV8Rk2";
        let desc_str = format!("ct({},elwpkh({}))", descriptor_blinding_key, xpub);
        let desc_str = format!("{}#{}", desc_str, desc_checksum(&desc_str).unwrap());
        let _desc = ConfidentialDescriptor::<DefiniteDescriptorKey>::from_str(&desc_str).unwrap();
    }

    fn new_wollet(desc: &str) -> Wollet {
        let desc = &format!("{}#{}", desc, desc_checksum(desc).unwrap());
        Wollet::new(
            ElementsNetwork::LiquidTestnet,
            "",
            false,
            false,
            "/tmp/.ks",
            desc,
        )
        .unwrap()
    }

    #[test]
    fn test_signers() {
        let view_key = "L3jXxwef3fpB7hcrFozcWgHeJCPSAFiZ1Ji2YJMPxceaGvy3PC1q";
        let xpub = "tpubDD7tXK8KeQ3YY83yWq755fHY2JW8Ha8Q765tknUM5rSvjPcGWfUppDFMpQ1ScziKfW3ZNtZvAD7M3u7bSs7HofjTD3KP3YxPK7X6hwV8Rk2";

        let fp1 = Fingerprint::from_str("11111111").unwrap();
        let fp2 = Fingerprint::from_str("22222222").unwrap();
        let fp_xpub = Fingerprint::from_str("0a55db61").unwrap();
        let fp_single = Fingerprint::from_str("51814f10").unwrap();

        let signer1 = format!("[{fp1}/0h/0h/0h]{xpub}/0/*");
        let signer1_mp = format!("[{fp1}/0h/0h/0h]{xpub}/<0;1>/*");
        let signer2 = format!("[{fp2}/0h/0h/0h]{xpub}/0/*");
        let signer_xpub = format!("{xpub}/0/*"); // no keyorigin
        let signer_single = "020202020202020202020202020202020202020202020202020202020202020202";

        let desc_s_1 = format!("ct({view_key},elwpkh({signer1}))");
        let desc_s_1mp = format!("ct({view_key},elwpkh({signer1_mp}))");
        let desc_s_xpub = format!("ct({view_key},elwpkh({signer_xpub}))");
        let desc_m_1single = format!("ct({view_key},elwsh(multi(2,{signer1},{signer_single})))");
        let desc_m_12 = format!("ct({view_key},elwsh(multi(2,{signer1},{signer2})))");

        assert_eq!(new_wollet(&desc_s_1).signers(), vec![fp1]);
        assert_eq!(new_wollet(&desc_s_1mp).signers(), vec![fp1]);
        assert_eq!(new_wollet(&desc_s_xpub).signers(), vec![fp_xpub]);
        assert_eq!(new_wollet(&desc_m_1single).signers(), vec![fp1, fp_single]);
        assert_eq!(new_wollet(&desc_m_12).signers(), vec![fp1, fp2]);
    }
}
