use crate::bitcoin::PublicKey as BitcoinPublicKey;
use crate::elements::confidential::AssetBlindingFactor;
use crate::elements::issuance::ContractHash;
use crate::elements::pset::{Input, Output, PartiallySignedTransaction};
use crate::elements::{Address, AssetId, OutPoint, Script, Transaction, TxOut, TxOutSecrets, Txid};
use crate::error::Error;
use crate::hashes::Hash;
use crate::model::{Addressee, UnvalidatedAddressee, WalletTxOut};
use crate::registry::Contract;
use crate::util::EC;
use crate::wallet::ElectrumWallet;
use elements_miniscript::psbt::PsbtExt;
use elements_miniscript::{DefiniteDescriptorKey, Descriptor};
use rand::thread_rng;
use std::collections::{HashMap, HashSet};
use std::str::FromStr;

// We make issuance and reissuance are mutually exclusive for simplicity
enum IssuanceRequest {
    None,
    Issuance(u64, u64, Option<Contract>),
    Reissuance(AssetId, u64),
}

impl ElectrumWallet {
    fn asset_utxos(&self, asset: &AssetId) -> Result<Vec<WalletTxOut>, Error> {
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
        let asset = self.validate_asset(addressee.asset)?;
        if addressee.address == "burn" {
            let burn_script = Script::new_op_return(&[]);
            Ok(Addressee {
                satoshi: addressee.satoshi,
                script_pubkey: burn_script,
                blinding_pubkey: None,
                asset,
            })
        } else {
            let address = self.validate_address(addressee.address)?;
            Ok(Addressee::from_address(addressee.satoshi, &address, asset))
        }
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

    fn add_output(
        &self,
        pset: &mut PartiallySignedTransaction,
        addressee: &Addressee,
    ) -> Result<(), Error> {
        let output = Output {
            script_pubkey: addressee.script_pubkey.clone(),
            amount: Some(addressee.satoshi),
            asset: Some(addressee.asset),
            blinding_key: addressee.blinding_pubkey.map(convert_pubkey),
            blinder_index: Some(0),
            ..Default::default()
        };
        pset.add_output(output);

        let last_output_index = pset.n_outputs() - 1;

        match self.definite_descriptor(&addressee.script_pubkey) {
            Ok(desc) => {
                pset.update_output_with_descriptor(last_output_index, &desc)
                    .map_err(|e| Error::Generic(e.to_string()))?; //TODO handle OutputUpdateError conversion
            }
            Err(Error::ScriptNotMine) => (),
            Err(e) => return Err(e),
        }

        Ok(())
    }

    fn add_input(
        &self,
        pset: &mut PartiallySignedTransaction,
        inp_txout_sec: &mut HashMap<usize, TxOutSecrets>,
        inp_weight: &mut usize,
        utxo: &WalletTxOut,
    ) -> Result<usize, Error> {
        let mut input = Input::from_prevout(utxo.outpoint);
        input.witness_utxo = Some(self.get_txout(&utxo.outpoint)?);
        input.non_witness_utxo = Some(self.get_tx(&utxo.outpoint.txid)?);

        pset.add_input(input);
        let idx = pset.inputs().len() - 1;
        let desc = self.definite_descriptor(&utxo.script_pubkey)?;
        pset.update_input_with_descriptor(idx, &desc)?;
        inp_txout_sec.insert(idx, utxo.unblinded);
        *inp_weight += desc.max_weight_to_satisfy()?;
        Ok(idx)
    }

    fn set_issuance(
        &self,
        pset: &mut PartiallySignedTransaction,
        idx: usize,
        satoshi_asset: u64,
        satoshi_token: u64,
        contract: Option<Contract>,
    ) -> Result<(AssetId, AssetId), Error> {
        let input = pset
            .inputs_mut()
            .get_mut(idx)
            .ok_or_else(|| Error::MissingVin)?;
        input.issuance_value_amount = Some(satoshi_asset);
        if satoshi_token > 0 {
            input.issuance_inflation_keys = Some(satoshi_token);
        }
        let prevout = OutPoint::new(input.previous_txid, input.previous_output_index);
        let contract_hash = match contract {
            Some(contract) => contract.contract_hash()?,
            None => ContractHash::from_slice(&[0u8; 32]).unwrap(),
        };
        let asset_entropy =
            Some(AssetId::generate_asset_entropy(prevout, contract_hash).to_byte_array());
        input.issuance_asset_entropy = asset_entropy;
        Ok(input.issuance_ids())
    }

    fn set_reissuance(
        &self,
        pset: &mut PartiallySignedTransaction,
        idx: usize,
        satoshi_asset: u64,
        token_asset_bf: &AssetBlindingFactor,
        entropy: &[u8; 32],
    ) -> Result<(), Error> {
        let input = pset
            .inputs_mut()
            .get_mut(idx)
            .ok_or_else(|| Error::MissingVin)?;
        input.issuance_value_amount = Some(satoshi_asset);
        let nonce = token_asset_bf.into_inner();
        input.issuance_blinding_nonce = Some(nonce);
        input.issuance_asset_entropy = Some(*entropy);
        Ok(())
    }

    fn addressee_change(
        &self,
        satoshi: u64,
        asset: AssetId,
        last_unused: &mut u32,
    ) -> Result<Addressee, Error> {
        let address = self.address(Some(*last_unused))?;
        *last_unused += 1;
        Ok(Addressee::from_address(satoshi, address.address(), asset))
    }

    fn createpset(
        &self,
        addressees: Vec<UnvalidatedAddressee>,
        fee_rate: Option<f32>,
        issuance_request: IssuanceRequest,
    ) -> Result<PartiallySignedTransaction, Error> {
        // Check user inputs
        let addressees = self.validate_addressees(addressees)?;
        let (addressees_lbtc, addressees_asset): (Vec<_>, Vec<_>) = addressees
            .into_iter()
            .partition(|a| a.asset == self.policy_asset());

        // Set fee rate (satoshi/Kvbytes)
        let fee_rate = fee_rate.unwrap_or(100.0);

        // Init PSET
        let mut pset = PartiallySignedTransaction::new_v2();
        let mut inp_txout_sec = HashMap::new();
        let mut last_unused = self.address(None)?.index();
        let mut inp_weight = 0;

        // Assets inputs and outputs
        let assets: HashSet<_> = addressees_asset.iter().map(|a| a.asset).collect();
        for asset in assets {
            let mut satoshi_out = 0;
            let mut satoshi_in = 0;
            for addressee in addressees_asset.iter().filter(|a| a.asset == asset) {
                self.add_output(&mut pset, addressee)?;
                satoshi_out += addressee.satoshi;
            }
            for utxo in self.asset_utxos(&asset)? {
                self.add_input(&mut pset, &mut inp_txout_sec, &mut inp_weight, &utxo)?;
                satoshi_in += utxo.unblinded.value;
                if satoshi_in >= satoshi_out {
                    if satoshi_in > satoshi_out {
                        let satoshi_change = satoshi_in - satoshi_out;
                        let addressee =
                            self.addressee_change(satoshi_change, asset, &mut last_unused)?;
                        self.add_output(&mut pset, &addressee)?;
                    }
                    break;
                }
            }
            if satoshi_in < satoshi_out {
                return Err(Error::InsufficientFunds);
            }
        }

        // L-BTC inputs and outputs
        // Fee and L-BTC change after (re)issuance
        let mut satoshi_out = 0;
        let mut satoshi_in = 0;
        for addressee in addressees_lbtc {
            self.add_output(&mut pset, &addressee)?;
            satoshi_out += addressee.satoshi;
        }

        // For implementation simplicity we always add all L-BTC inputs
        for utxo in self.asset_utxos(&self.policy_asset())? {
            self.add_input(&mut pset, &mut inp_txout_sec, &mut inp_weight, &utxo)?;
            satoshi_in += utxo.unblinded.value;
        }

        // Set (re)issuance data
        match issuance_request {
            IssuanceRequest::None => {}
            IssuanceRequest::Issuance(satoshi_asset, satoshi_token, contract) => {
                // At least a L-BTC input for the fee was added.
                let idx = 0;
                let (asset, token) =
                    self.set_issuance(&mut pset, idx, satoshi_asset, satoshi_token, contract)?;

                let addressee = self.addressee_change(satoshi_asset, asset, &mut last_unused)?;
                self.add_output(&mut pset, &addressee)?;

                if satoshi_token > 0 {
                    let addressee =
                        self.addressee_change(satoshi_token, token, &mut last_unused)?;
                    self.add_output(&mut pset, &addressee)?;
                }
            }
            IssuanceRequest::Reissuance(asset, satoshi_asset) => {
                let issuance = self.issuance(&asset)?;
                let token = issuance.token;
                // Find or add input for the token
                let (idx, token_asset_bf) =
                    match inp_txout_sec.iter().find(|(_, u)| u.asset == token) {
                        Some((idx, u)) => (*idx, u.asset_bf),
                        None => {
                            // Add an input sending the token,
                            let utxos_token = self.asset_utxos(&token)?;
                            let utxo_token = utxos_token
                                .first()
                                .ok_or_else(|| Error::InsufficientFunds)?;
                            let idx = self.add_input(
                                &mut pset,
                                &mut inp_txout_sec,
                                &mut inp_weight,
                                utxo_token,
                            )?;

                            // and an outpout receiving the token
                            let satoshi_token = utxo_token.unblinded.value;
                            let addressee =
                                self.addressee_change(satoshi_token, token, &mut last_unused)?;
                            self.add_output(&mut pset, &addressee)?;

                            (idx, utxo_token.unblinded.asset_bf)
                        }
                    };

                // Set reissuance data
                self.set_reissuance(
                    &mut pset,
                    idx,
                    satoshi_asset,
                    &token_asset_bf,
                    &issuance.entropy,
                )?;

                let addressee = self.addressee_change(satoshi_asset, asset, &mut last_unused)?;
                self.add_output(&mut pset, &addressee)?;
            }
        }

        // Add a temporary fee, and always add a change output,
        // then we'll tweak those values to match the given fee rate.
        let temp_fee = 1000;
        if satoshi_in < (satoshi_out + temp_fee) {
            return Err(Error::InsufficientFunds);
        }
        let satoshi_change = satoshi_in - satoshi_out - temp_fee;
        let addressee =
            self.addressee_change(satoshi_change, self.policy_asset(), &mut last_unused)?;
        self.add_output(&mut pset, &addressee)?;
        let fee_output =
            Output::new_explicit(Script::default(), temp_fee, self.policy_asset(), None);
        pset.add_output(fee_output);

        let weight = {
            let mut rng = thread_rng();
            let mut temp_pset = pset.clone();
            temp_pset.blind_last(&mut rng, &EC, &inp_txout_sec)?;
            inp_weight + temp_pset.extract_tx()?.weight()
        };

        let vsize = (weight + 4 - 1) / 4;
        let fee = (vsize as f32 * fee_rate / 1000.0).ceil() as u64;
        if satoshi_in < (satoshi_out + temp_fee) {
            return Err(Error::InsufficientFunds);
        }
        let satoshi_change = satoshi_in - satoshi_out - fee;
        // Replace change and fee outputs
        let n_outputs = pset.n_outputs();
        let outputs = pset.outputs_mut();
        let change_output = &mut outputs[n_outputs - 2];
        change_output.amount = Some(satoshi_change);
        let fee_output = &mut outputs[n_outputs - 1];
        fee_output.amount = Some(fee);

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
        fee_rate: Option<f32>,
    ) -> Result<PartiallySignedTransaction, Error> {
        let addressees = vec![UnvalidatedAddressee {
            satoshi,
            address,
            asset: "",
        }];
        self.createpset(addressees, fee_rate, IssuanceRequest::None)
    }

    /// Create a PSET sending some satoshi of an asset to an address
    pub fn sendasset(
        &self,
        satoshi: u64,
        address: &str,
        asset: &str,
        fee_rate: Option<f32>,
    ) -> Result<PartiallySignedTransaction, Error> {
        let addressees = vec![UnvalidatedAddressee {
            satoshi,
            address,
            asset,
        }];
        self.createpset(addressees, fee_rate, IssuanceRequest::None)
    }

    /// Create a PSET sending to many outputs
    pub fn sendmany(
        &self,
        addressees: Vec<UnvalidatedAddressee>,
        fee_rate: Option<f32>,
    ) -> Result<PartiallySignedTransaction, Error> {
        self.createpset(addressees, fee_rate, IssuanceRequest::None)
    }

    /// Create a PSET burning an asset
    pub fn burnasset(
        &self,
        asset: &str,
        satoshi: u64,
        fee_rate: Option<f32>,
    ) -> Result<PartiallySignedTransaction, Error> {
        let addressees = vec![UnvalidatedAddressee {
            satoshi,
            address: "burn",
            asset,
        }];
        self.createpset(addressees, fee_rate, IssuanceRequest::None)
    }

    /// Create a PSET issuing an asset
    pub fn issueasset(
        &self,
        satoshi_asset: u64,
        satoshi_token: u64,
        contract: &str,
        fee_rate: Option<f32>,
    ) -> Result<PartiallySignedTransaction, Error> {
        let addressees = vec![];
        let contract = if contract.is_empty() {
            None
        } else {
            let contract = serde_json::Value::from_str(contract)?;
            let contract = Contract::from_value(&contract)?;
            contract.validate()?;
            Some(contract)
        };
        let issuance = IssuanceRequest::Issuance(satoshi_asset, satoshi_token, contract);
        self.createpset(addressees, fee_rate, issuance)
    }

    /// Create a PSET reissuing an asset
    pub fn reissueasset(
        &self,
        asset: &str,
        satoshi_asset: u64,
        fee_rate: Option<f32>,
    ) -> Result<PartiallySignedTransaction, Error> {
        let addressees = vec![];
        let asset = AssetId::from_str(asset)?;
        let reissuance = IssuanceRequest::Reissuance(asset, satoshi_asset);
        self.createpset(addressees, fee_rate, reissuance)
    }
}

fn convert_pubkey(pk: crate::elements::secp256k1_zkp::PublicKey) -> BitcoinPublicKey {
    BitcoinPublicKey::new(pk)
}
