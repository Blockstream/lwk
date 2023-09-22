use crate::bitcoin::PublicKey as BitcoinPublicKey;
use crate::config::{Config, ElementsNetwork};
use crate::elements::encode::{
    deserialize as elements_deserialize, serialize as elements_serialize,
};
use crate::elements::issuance::ContractHash;
use crate::elements::pset::{Input, Output, PartiallySignedTransaction};
use crate::elements::{
    Address, AddressParams, AssetId, BlockHash, BlockHeader, OutPoint, Script, Transaction, TxOut,
    Txid,
};
use crate::error::Error;
use crate::hashes::{sha256, Hash};
use crate::model::{AddressResult, Addressee, UnblindedTXO, UnvalidatedAddressee, TXO};
use crate::pset_details::{pset_balance, PsetBalance};
use crate::store::{new_store, Store};
use crate::sync::sync;
use crate::util::EC;
use electrum_client::bitcoin::bip32::ChildNumber;
use electrum_client::ElectrumApi;
use elements_miniscript::confidential::Key;
use elements_miniscript::psbt;
use elements_miniscript::psbt::PsbtExt;
use elements_miniscript::{
    ConfidentialDescriptor, DefiniteDescriptorKey, Descriptor, DescriptorPublicKey,
};
use rand::thread_rng;
use std::cmp::Ordering;
use std::collections::HashMap;
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::atomic;

pub(crate) fn derive_address(
    descriptor: &ConfidentialDescriptor<DescriptorPublicKey>,
    index: u32,
    address_params: &'static AddressParams,
) -> Result<Address, Error> {
    let derived_non_conf = descriptor.descriptor.at_derivation_index(index)?;

    let derived_conf = ConfidentialDescriptor::<DefiniteDescriptorKey> {
        key: convert_blinding_key(&descriptor.key)?,
        descriptor: derived_non_conf,
    };

    Ok(derived_conf.address(&EC, address_params)?)
}

pub(crate) fn derive_script_pubkey(
    descriptor: &ConfidentialDescriptor<DescriptorPublicKey>,
    index: u32,
) -> Result<Script, Error> {
    Ok(derive_address(descriptor, index, &AddressParams::LIQUID)?.script_pubkey())
}

fn convert_blinding_key(
    key: &Key<DescriptorPublicKey>,
) -> Result<Key<DefiniteDescriptorKey>, Error> {
    match key {
        Key::Slip77(x) => Ok(Key::Slip77(*x)),
        Key::Bare(_) => Err(Error::BlindingBareUnsupported),
        Key::View(x) => Ok(Key::View(x.clone())),
    }
}

pub struct ElectrumWallet {
    config: Config,
    store: Store,
    descriptor: ConfidentialDescriptor<DescriptorPublicKey>,
}

impl ElectrumWallet {
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
        let descriptor = ConfidentialDescriptor::<DescriptorPublicKey>::from_str(desc)?;

        let wallet_desc = format!("{}{:?}", desc, config);
        let wallet_id = format!("{}", sha256::Hash::hash(wallet_desc.as_bytes()));

        let mut path: PathBuf = config.data_root().into();
        if !path.exists() {
            std::fs::create_dir_all(&path)?;
        }
        path.push(wallet_id);
        let store = new_store(&path, descriptor.clone())?;

        Ok(ElectrumWallet {
            store,
            config,
            descriptor,
        })
    }

    fn descriptor_blinding_key(&self) -> Key<DefiniteDescriptorKey> {
        convert_blinding_key(&self.descriptor.key)
            .expect("No private blinding keys for bare variant")
    }

    /// Get the network policy asset
    pub fn policy_asset(&self) -> AssetId {
        self.config.policy_asset()
    }

    /// Get the wallet descriptor
    pub fn descriptor(&self) -> &ConfidentialDescriptor<DescriptorPublicKey> {
        &self.descriptor
    }

    /// Sync the wallet transactions
    pub fn sync_txs(&mut self) -> Result<(), Error> {
        if let Ok(client) = self.config.electrum_url().build_client() {
            let blinding_key = self.descriptor_blinding_key();
            match sync(&client, &mut self.store, blinding_key) {
                Ok(true) => log::info!("there are new transcations"),
                Ok(false) => (),
                Err(e) => log::warn!("Error during sync, {:?}", e),
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

    fn derive_address(&self, index: u32) -> Result<Address, Error> {
        derive_address(&self.descriptor, index, self.config.address_params())
    }

    /// Get a wallet address
    ///
    /// If Some return the address at the given index,
    /// otherwise the last unused address.
    pub fn address(&self, index: Option<u32>) -> Result<AddressResult, Error> {
        let index = match index {
            Some(i) => i,
            None => self.store.cache.last_unused.load(atomic::Ordering::Relaxed),
        };

        Ok(AddressResult::new(self.derive_address(index)?, index))
    }

    /// Get the wallet UTXOs
    pub fn utxos(&self) -> Result<Vec<UnblindedTXO>, Error> {
        let mut txos = vec![];
        let spent = self.store.spent()?;
        for (tx_id, height) in self.store.cache.heights.iter() {
            let tx = self
                .store
                .cache
                .all_txs
                .get(tx_id)
                .ok_or_else(|| Error::Generic(format!("txos no tx {}", tx_id)))?;
            let tx_txos: Vec<UnblindedTXO> = {
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
                            let txo = TXO::new(outpoint, output.script_pubkey, *height);
                            return Some(UnblindedTXO {
                                txo,
                                unblinded: *unblinded,
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

    fn balance_from_utxos(&self, utxos: &[UnblindedTXO]) -> Result<HashMap<AssetId, u64>, Error> {
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

    /// Get the PSET details with respect to the wallet
    pub fn get_details(&self, pset: &PartiallySignedTransaction) -> Result<PsetBalance, Error> {
        Ok(pset_balance(
            pset,
            &self.store.cache.unblinded,
            self.descriptor(),
        )?)
    }

    #[allow(dead_code)]
    fn asset_utxos(&self, asset: &AssetId) -> Result<Vec<UnblindedTXO>, Error> {
        Ok(self
            .utxos()?
            .into_iter()
            .filter(|utxo| &utxo.unblinded.asset == asset)
            .collect())
    }

    fn get_tx(&self, txid: &Txid) -> Result<Transaction, Error> {
        Ok(self
            .store
            .cache
            .all_txs
            .get(txid)
            .ok_or_else(|| Error::MissingTransaction)?
            .clone())
    }

    fn get_txout(&self, outpoint: &OutPoint) -> Result<TxOut, Error> {
        Ok(self
            .get_tx(&outpoint.txid)?
            .output
            .get(outpoint.vout as usize)
            .ok_or_else(|| Error::MissingVout)?
            .clone())
    }

    fn index(&self, script_pubkey: &Script) -> Result<u32, Error> {
        let index = self
            .store
            .cache
            .paths
            .get(script_pubkey)
            .ok_or_else(|| Error::ScriptNotMine)?;
        match index {
            ChildNumber::Normal { index } => Ok(*index),
            ChildNumber::Hardened { index: _ } => {
                Err(Error::Generic("unexpected hardened derivation".into()))
            }
        }
    }

    fn definite_descriptor(
        &self,
        script_pubkey: &Script,
    ) -> Result<Descriptor<DefiniteDescriptorKey>, Error> {
        let utxo_index = self.index(script_pubkey)?;
        Ok(self.descriptor.descriptor.at_derivation_index(utxo_index)?)
    }

    fn validate_address(&self, address: &str) -> Result<Address, Error> {
        let params = self.config.address_params();
        let address = Address::parse_with_params(address, params)?;
        if address.blinding_pubkey.is_none() {
            return Err(Error::NotConfidentialAddress);
        };
        Ok(address)
    }

    fn validate_asset(&self, asset: &str) -> Result<AssetId, Error> {
        if asset.is_empty() {
            Ok(self.policy_asset())
        } else {
            Ok(AssetId::from_str(asset)?)
        }
    }

    fn validate_addressee(&self, addressee: &UnvalidatedAddressee) -> Result<Addressee, Error> {
        let address = self.validate_address(addressee.address)?;
        let asset = self.validate_asset(addressee.asset)?;
        Ok(Addressee {
            satoshi: addressee.satoshi,
            address,
            asset,
        })
    }

    fn validate_addressees(
        &self,
        addressees: Vec<UnvalidatedAddressee>,
    ) -> Result<Vec<Addressee>, Error> {
        addressees
            .iter()
            .map(|a| self.validate_addressee(a))
            .collect()
    }

    fn tot_out(&self, addressees: &Vec<Addressee>) -> Result<HashMap<AssetId, u64>, Error> {
        let mut r = HashMap::new();
        r.entry(self.policy_asset()).or_insert(0);
        for addressee in addressees {
            *r.entry(addressee.asset).or_default() += addressee.satoshi;
        }
        Ok(r)
    }

    fn add_output(
        &self,
        pset: &mut PartiallySignedTransaction,
        addressee: &Addressee,
    ) -> Result<(), Error> {
        let output = Output {
            script_pubkey: addressee.address.script_pubkey(),
            amount: Some(addressee.satoshi),
            asset: Some(addressee.asset),
            blinding_key: addressee.address.blinding_pubkey.map(convert_pubkey),
            blinder_index: Some(0),
            ..Default::default()
        };
        pset.add_output(output);

        let last_output_index = pset.n_outputs() - 1;

        match self.definite_descriptor(&addressee.address.script_pubkey()) {
            Ok(desc) => {
                pset.update_output_with_descriptor(last_output_index, &desc)
                    .map_err(|e| Error::Generic(e.to_string()))?; //TODO handle OutputUpdateError conversion
            }
            Err(Error::ScriptNotMine) => (),
            Err(e) => return Err(e),
        }

        Ok(())
    }

    fn createpset(
        &self,
        addressees: Vec<UnvalidatedAddressee>,
        fee: Option<u64>,
    ) -> Result<PartiallySignedTransaction, Error> {
        // Check user inputs
        let addressees = self.validate_addressees(addressees)?;

        // Get utxos
        let utxos = self.utxos()?;

        // Set fee
        let fee = fee.unwrap_or(1_000);

        // Check if we have enough funds and compute change
        let tot_in = self.balance_from_utxos(&utxos)?;
        let mut tot_out = self.tot_out(&addressees)?;
        *tot_out.entry(self.policy_asset()).or_default() += fee;

        let mut last_unused = self.address(None)?.index();
        let mut addressees_change = vec![];
        for (asset, satoshi_out) in tot_out.clone() {
            let satoshi_in: u64 = *tot_in.get(&asset).unwrap_or(&0);
            if satoshi_in < satoshi_out {
                return Err(Error::InsufficientFunds);
            }
            let satoshi_change = satoshi_in - satoshi_out;
            if satoshi_change > 0 {
                let address_change = self.address(Some(last_unused))?;
                last_unused += 1;
                addressees_change.push(Addressee {
                    satoshi: satoshi_change,
                    address: address_change.address().clone(),
                    asset,
                });
            }
        }

        // Init PSET
        let mut pset = PartiallySignedTransaction::new_v2();
        let mut inp_txout_sec = HashMap::new();

        // Add inputs
        for (idx, utxo) in utxos.iter().enumerate() {
            if tot_out.get(&utxo.unblinded.asset).is_none() {
                // Do not add utxos if the we are not sending the asset
                continue;
            }
            let mut input = Input::from_prevout(utxo.txo.outpoint);
            input.witness_utxo = Some(self.get_txout(&utxo.txo.outpoint)?);
            input.non_witness_utxo = Some(self.get_tx(&utxo.txo.outpoint.txid)?);

            pset.add_input(input);
            let desc = self.definite_descriptor(&utxo.txo.script_pubkey)?;
            pset.update_input_with_descriptor(idx, &desc)?;
            inp_txout_sec.insert(idx, utxo.unblinded);
        }

        // Add outputs
        for addressee in addressees {
            self.add_output(&mut pset, &addressee)?;
        }
        for addressee in addressees_change {
            self.add_output(&mut pset, &addressee)?;
        }
        let fee_output = Output::new_explicit(Script::default(), fee, self.policy_asset(), None);
        pset.add_output(fee_output);

        // Blind the transaction
        let mut rng = thread_rng();
        pset.blind_last(&mut rng, &EC, &inp_txout_sec)?;
        Ok(pset)
    }

    /// Create a PSET sending some satoshi to an address
    pub fn sendlbtc(
        &self,
        satoshi: u64,
        address: &str,
    ) -> Result<PartiallySignedTransaction, Error> {
        let addressees = vec![UnvalidatedAddressee {
            satoshi,
            address,
            asset: "",
        }];
        self.createpset(addressees, None)
    }

    /// Create a PSET sending some satoshi of an asset to an address
    pub fn sendasset(
        &self,
        satoshi: u64,
        address: &str,
        asset: &str,
    ) -> Result<PartiallySignedTransaction, Error> {
        let addressees = vec![UnvalidatedAddressee {
            satoshi,
            address,
            asset,
        }];
        self.createpset(addressees, None)
    }

    /// Create a PSET sending to many outputs
    pub fn sendmany(
        &self,
        addressees: Vec<UnvalidatedAddressee>,
    ) -> Result<PartiallySignedTransaction, Error> {
        self.createpset(addressees, None)
    }

    /// Create a PSET issuing an asset
    pub fn issueasset(
        &self,
        satoshi_asset: u64,
        satoshi_token: u64,
    ) -> Result<PartiallySignedTransaction, Error> {
        // Get utxos
        let utxos = self.asset_utxos(&self.policy_asset())?;
        let utxo = utxos[0].clone();

        // Set fee
        let fee = 1_000;

        // Init PSET
        let mut pset = PartiallySignedTransaction::new_v2();
        let mut inp_txout_sec = HashMap::new();

        // Add a policy asset input
        let mut input = Input::from_prevout(utxo.txo.outpoint);
        input.witness_utxo = Some(self.get_txout(&utxo.txo.outpoint)?);
        input.non_witness_utxo = Some(self.get_tx(&utxo.txo.outpoint.txid)?);

        // Set issuance data
        input.issuance_value_amount = Some(satoshi_asset);
        if satoshi_token > 0 {
            input.issuance_inflation_keys = Some(satoshi_token);
        }
        let prevout = OutPoint::new(input.previous_txid, input.previous_output_index);
        let contract_hash = ContractHash::from_slice(&[0u8; 32]).unwrap();
        let asset_entropy =
            Some(AssetId::generate_asset_entropy(prevout, contract_hash).to_byte_array());
        input.issuance_asset_entropy = asset_entropy;
        let (asset, token) = input.issuance_ids();

        pset.add_input(input);
        let idx = 0;
        let desc = self.definite_descriptor(&utxo.txo.script_pubkey)?;
        pset.update_input_with_descriptor(idx, &desc)?;
        inp_txout_sec.insert(idx, utxo.unblinded);
        let satoshi_change = utxo.unblinded.value - fee;

        // Add outputs
        let mut last_unused = self.address(None)?.index();
        let mut addressees = vec![];
        addressees.push(Addressee {
            satoshi: satoshi_asset,
            address: self.address(Some(last_unused))?.address().clone(),
            asset,
        });
        last_unused += 1;
        if satoshi_token > 0 {
            addressees.push(Addressee {
                satoshi: satoshi_token,
                address: self.address(Some(last_unused))?.address().clone(),
                asset: token,
            });
            last_unused += 1;
        }
        addressees.push(Addressee {
            satoshi: satoshi_change,
            address: self.address(Some(last_unused))?.address().clone(),
            asset: self.policy_asset(),
        });

        for addressee in addressees {
            self.add_output(&mut pset, &addressee)?;
        }
        let fee_output = Output::new_explicit(Script::default(), fee, self.policy_asset(), None);
        pset.add_output(fee_output);

        // Blind the transaction
        let mut rng = thread_rng();
        pset.blind_last(&mut rng, &EC, &inp_txout_sec)?;
        Ok(pset)
    }

    /// Create a PSET reissuing an asset
    pub fn reissueasset(
        &self,
        entropy: &str,
        satoshi_asset: u64,
    ) -> Result<PartiallySignedTransaction, Error> {
        let entropy = sha256::Midstate::from_str(entropy).unwrap();
        let asset = AssetId::from_entropy(entropy);
        let confidential = false; // FIXME
        let token = AssetId::reissuance_token_from_entropy(entropy, confidential);

        // Get utxos
        let utxos_token = self.asset_utxos(&token)?;
        let utxo_token = utxos_token[0].clone();
        let utxos_btc = self.asset_utxos(&self.policy_asset())?;
        let utxo_btc = utxos_btc[0].clone();

        // Set fee
        let fee = 1_000;

        // Init PSET
        let mut pset = PartiallySignedTransaction::new_v2();
        let mut inp_txout_sec = HashMap::new();

        // Add the reissuance token input
        let mut input = Input::from_prevout(utxo_token.txo.outpoint);
        input.witness_utxo = Some(self.get_txout(&utxo_token.txo.outpoint)?);
        input.non_witness_utxo = Some(self.get_tx(&utxo_token.txo.outpoint.txid)?);

        let satoshi_token = utxo_token.unblinded.value;

        // Set issuance data
        input.issuance_value_amount = Some(satoshi_asset);
        let nonce = utxo_token.unblinded.asset_bf.into_inner();
        input.issuance_blinding_nonce = Some(nonce);
        input.issuance_asset_entropy = Some(entropy.to_byte_array());

        pset.add_input(input);
        let idx = 0;
        let desc = self.definite_descriptor(&utxo_token.txo.script_pubkey)?;
        pset.update_input_with_descriptor(idx, &desc)?;
        inp_txout_sec.insert(idx, utxo_token.unblinded);

        // Add a policy asset input
        let mut input = Input::from_prevout(utxo_btc.txo.outpoint);
        input.witness_utxo = Some(self.get_txout(&utxo_btc.txo.outpoint)?);
        input.non_witness_utxo = Some(self.get_tx(&utxo_btc.txo.outpoint.txid)?);

        pset.add_input(input);
        let idx = 1;
        let desc = self.definite_descriptor(&utxo_btc.txo.script_pubkey)?;
        pset.update_input_with_descriptor(idx, &desc)?;
        inp_txout_sec.insert(idx, utxo_btc.unblinded);
        let satoshi_change = utxo_btc.unblinded.value - fee;

        // Add outputs
        let mut last_unused = self.address(None)?.index();
        let mut addressees = vec![];
        addressees.push(Addressee {
            satoshi: satoshi_asset,
            address: self.address(Some(last_unused))?.address().clone(),
            asset,
        });
        last_unused += 1;
        if satoshi_token > 0 {
            addressees.push(Addressee {
                satoshi: satoshi_token,
                address: self.address(Some(last_unused))?.address().clone(),
                asset: token,
            });
            last_unused += 1;
        }
        addressees.push(Addressee {
            satoshi: satoshi_change,
            address: self.address(Some(last_unused))?.address().clone(),
            asset: self.policy_asset(),
        });

        for addressee in addressees {
            self.add_output(&mut pset, &addressee)?;
        }
        let fee_output = Output::new_explicit(Script::default(), fee, self.policy_asset(), None);
        pset.add_output(fee_output);

        // Blind the transaction
        let mut rng = thread_rng();
        pset.blind_last(&mut rng, &EC, &inp_txout_sec)?;
        Ok(pset)
    }

    /// Create a PSET burning an asset
    pub fn burnasset(
        &self,
        asset: &str,
        satoshi_asset: u64,
    ) -> Result<PartiallySignedTransaction, Error> {
        let asset = AssetId::from_str(asset)?;

        // Get utxos
        let mut utxos = self.asset_utxos(&asset)?;
        let tot_asset: u64 = utxos.iter().map(|u| u.unblinded.value).sum();
        let mut utxos_btc = self.asset_utxos(&self.policy_asset())?;
        let tot_btc: u64 = utxos_btc.iter().map(|u| u.unblinded.value).sum();
        utxos.append(&mut utxos_btc);

        if tot_asset < satoshi_asset {
            return Err(Error::InsufficientFunds);
        }
        let satoshi_change = tot_asset - satoshi_asset;

        // Set fee
        let fee = 1_000;
        if tot_btc < fee {
            return Err(Error::InsufficientFunds);
        }
        let satoshi_btc = tot_btc - fee;

        // Init PSET
        let mut pset = PartiallySignedTransaction::new_v2();
        let mut inp_txout_sec = HashMap::new();

        // Add inputs
        for (idx, utxo) in utxos.iter().enumerate() {
            let mut input = Input::from_prevout(utxo.txo.outpoint);
            input.witness_utxo = Some(self.get_txout(&utxo.txo.outpoint)?);
            input.non_witness_utxo = Some(self.get_tx(&utxo.txo.outpoint.txid)?);
            pset.add_input(input);
            let desc = self.definite_descriptor(&utxo.txo.script_pubkey)?;
            pset.update_input_with_descriptor(idx, &desc)?;
            inp_txout_sec.insert(idx, utxo.unblinded);
        }

        // Add outputs
        let burn_script = Script::new_op_return(&[]);
        let burn_output = Output::new_explicit(burn_script, satoshi_asset, asset, None);
        pset.add_output(burn_output);

        // Add outputs
        let mut last_unused = self.address(None)?.index();
        let mut addressees = vec![];
        if satoshi_asset > 0 {
            addressees.push(Addressee {
                satoshi: satoshi_change,
                address: self.address(Some(last_unused))?.address().clone(),
                asset,
            });
            last_unused += 1;
        }
        if satoshi_btc > 0 {
            addressees.push(Addressee {
                satoshi: satoshi_btc,
                address: self.address(Some(last_unused))?.address().clone(),
                asset: self.policy_asset(),
            });
        }

        for addressee in addressees {
            self.add_output(&mut pset, &addressee)?;
        }
        let fee_output = Output::new_explicit(Script::default(), fee, self.policy_asset(), None);
        pset.add_output(fee_output);

        // Blind the transaction
        let mut rng = thread_rng();
        pset.blind_last(&mut rng, &EC, &inp_txout_sec)?;
        Ok(pset)
    }

    pub fn finalize(&self, pset: &mut PartiallySignedTransaction) -> Result<Transaction, Error> {
        // genesis_hash is only used for BIP341 (taproot) sighash computation
        psbt::finalize(pset, &EC, BlockHash::all_zeros()).unwrap();
        Ok(pset.extract_tx()?)
    }

    pub fn broadcast(&self, tx: &Transaction) -> Result<Txid, Error> {
        let client = self.config.electrum_url().build_client()?;
        let txid = client.transaction_broadcast_raw(&elements_serialize(tx))?;
        Ok(Txid::from_raw_hash(txid.to_raw_hash()))
    }
}

fn convert_pubkey(pk: crate::elements::secp256k1_zkp::PublicKey) -> BitcoinPublicKey {
    BitcoinPublicKey::new(pk)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::elements::bitcoin::bip32::{ExtendedPrivKey, ExtendedPubKey};
    use crate::elements::bitcoin::network::constants::Network;
    use elements_miniscript::confidential::bare::tweak_private_key;
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
    fn test_address_from_desc_wildcard() {
        let xpub = "tpubDC2Q4xK4XH72GLdvD62W5NsFiD3HmTScXpopTsf3b4AUqkQwBd7wmWAJki61sov1MVuyU4MuGLJHF7h3j1b3e1FY2wvUVVx7vagmxdPvVsv";
        let master_blinding_key =
            "9c8e4f05c7711a98c838be228bcb84924d4570ca53f35fa1c793e58841d47023";
        let checksum = "yfhwtmd8";
        let desc_str = format!(
            "ct(slip77({}),elsh(wpkh({}/0/*)))#{}",
            master_blinding_key, xpub, checksum
        );
        let desc = ConfidentialDescriptor::<DescriptorPublicKey>::from_str(&desc_str).unwrap();

        let addr = derive_address(&desc, 0, &AddressParams::LIQUID_TESTNET).unwrap();
        let expected_addr =
            "vjTwLVioiKrDJ7zZZn9iQQrxP6RPpcvpHBhzZrbdZKKVZE29FuXSnkXdKcxK3qD5t1rYsdxcm9KYRMji";
        assert_eq!(addr.to_string(), expected_addr.to_string());

        let addr = derive_address(&desc, 1, &AddressParams::LIQUID_TESTNET).unwrap();
        let expected_addr =
            "vjTuhaPWWbywbSy2EeRWWQ8bN2pPLmM4gFQTkA7DPX7uaCApKuav1e6LW1GKHuLUHdbv9Eag5MybsZoy";
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
}
