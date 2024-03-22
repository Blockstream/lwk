use crate::bitcoin::PublicKey as BitcoinPublicKey;
use crate::elements::confidential::AssetBlindingFactor;
use crate::elements::issuance::ContractHash;
use crate::elements::pset::{Input, Output, PartiallySignedTransaction};
use crate::elements::{Address, AssetId, OutPoint, Transaction, TxOut, TxOutSecrets, Txid};
use crate::error::Error;
use crate::hashes::Hash;
use crate::model::{Recipient, UnvalidatedRecipient, WalletTxOut};
use crate::registry::Contract;
use crate::tx_builder::TxBuilder;
use crate::wollet::Wollet;
use crate::ElementsNetwork;
use elements::pset::elip100::AssetMetadata;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::str::FromStr;

#[derive(Debug, Serialize, Deserialize)]
// We make issuance and reissuance are mutually exclusive for simplicity
pub enum IssuanceRequest {
    None,
    Issuance(u64, Option<Address>, u64, Option<Address>, Option<Contract>),
    Reissuance(AssetId, u64, Option<Address>),
}

impl Wollet {
    pub(crate) fn asset_utxos(&self, asset: &AssetId) -> Result<Vec<WalletTxOut>, Error> {
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

    pub(crate) fn add_output(
        &self,
        pset: &mut PartiallySignedTransaction,
        addressee: &Recipient,
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

        Ok(())
    }

    pub(crate) fn add_input(
        &self,
        pset: &mut PartiallySignedTransaction,
        inp_txout_sec: &mut HashMap<usize, TxOutSecrets>,
        inp_weight: &mut usize,
        utxo: &WalletTxOut,
    ) -> Result<usize, Error> {
        let mut input = Input::from_prevout(utxo.outpoint);
        let mut txout = self.get_txout(&utxo.outpoint)?;
        // This field is used by stateless blinders or signers to
        // learn the blinding factors and unblinded values of this input.
        // We need this since the output witness, which includes the
        // rangeproof, is not serialized.
        // Note that we explicitly remove the txout rangeproof to avoid
        // relying on its presence.
        input.in_utxo_rangeproof = txout.witness.rangeproof.take();
        input.witness_utxo = Some(txout);

        pset.add_input(input);
        let idx = pset.inputs().len() - 1;
        let desc = self.definite_descriptor(&utxo.script_pubkey)?;
        inp_txout_sec.insert(idx, utxo.unblinded);
        *inp_weight += desc.max_weight_to_satisfy()?;
        Ok(idx)
    }

    pub(crate) fn set_issuance(
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
        let contract_hash = match contract.as_ref() {
            Some(contract) => contract.contract_hash()?,
            None => ContractHash::from_slice(&[0u8; 32]).expect("static"),
        };
        input.issuance_asset_entropy = Some(contract_hash.to_byte_array());

        let (asset, token) = input.issuance_ids();

        if let Some(contract) = contract.as_ref() {
            let issuance_prevout = OutPoint::new(input.previous_txid, input.previous_output_index);
            let contract = serde_json::to_string(&contract)?;
            pset.add_asset_metadata(asset, &AssetMetadata::new(contract, issuance_prevout));
        }

        Ok((asset, token))
    }

    pub(crate) fn set_reissuance(
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

    pub(crate) fn addressee_change(
        &self,
        satoshi: u64,
        asset: AssetId,
        last_unused: &mut u32,
    ) -> Result<Recipient, Error> {
        let address = self.change(Some(*last_unused))?;
        *last_unused += 1;
        Ok(Recipient::from_address(satoshi, address.address(), asset))
    }

    pub(crate) fn addressee_external(
        &self,
        satoshi: u64,
        asset: AssetId,
        last_unused: &mut u32,
    ) -> Result<Recipient, Error> {
        let address = self.address(Some(*last_unused))?;
        *last_unused += 1;
        Ok(Recipient::from_address(satoshi, address.address(), asset))
    }

    /// Create a PSET burning an asset
    pub fn burn_asset(
        &self,
        asset: &str,
        satoshi: u64,
        fee_rate: Option<f32>,
    ) -> Result<PartiallySignedTransaction, Error> {
        let recipient = UnvalidatedRecipient::burn(asset.to_string(), satoshi);
        TxBuilder::new(self.network())
            .add_unvalidated_recipient(&recipient)?
            .fee_rate(fee_rate)
            .finish(self)
    }

    /// Create a PSET issuing an asset
    pub fn issue_asset(
        &self,
        satoshi_asset: u64,
        address_asset: &str,
        satoshi_token: u64,
        address_token: &str,
        contract: &str,
        fee_rate: Option<f32>,
    ) -> Result<PartiallySignedTransaction, Error> {
        let contract = if contract.is_empty() {
            None
        } else {
            let contract = serde_json::Value::from_str(contract)?;
            let contract = Contract::from_value(&contract)?;
            contract.validate()?;
            Some(contract)
        };
        let address_asset = validate_empty_address(address_asset, self.network())?;
        let address_token = validate_empty_address(address_token, self.network())?;
        TxBuilder::new(self.network())
            .fee_rate(fee_rate)
            .issue_asset(
                satoshi_asset,
                address_asset,
                satoshi_token,
                address_token,
                contract,
            )?
            .finish(self)
    }

    /// Create a PSET reissuing an asset
    pub fn reissue_asset(
        &self,
        asset: &str,
        satoshi_asset: u64,
        address_asset: &str,
        fee_rate: Option<f32>,
    ) -> Result<PartiallySignedTransaction, Error> {
        let asset = AssetId::from_str(asset)?;
        let address_asset = validate_empty_address(address_asset, self.network())?;
        TxBuilder::new(self.network())
            .fee_rate(fee_rate)
            .reissue_asset(asset, satoshi_asset, address_asset)?
            .finish(self)
    }
}

fn convert_pubkey(pk: crate::elements::secp256k1_zkp::PublicKey) -> BitcoinPublicKey {
    BitcoinPublicKey::new(pk)
}

pub(crate) fn validate_address(address: &str, network: ElementsNetwork) -> Result<Address, Error> {
    let params = network.address_params();
    let address = Address::parse_with_params(address, params)?;
    if address.blinding_pubkey.is_none() {
        return Err(Error::NotConfidentialAddress);
    };
    Ok(address)
}

pub(crate) fn validate_empty_address(
    address: &str,
    network: ElementsNetwork,
) -> Result<Option<Address>, Error> {
    (!address.is_empty())
        .then(|| validate_address(address, network))
        .transpose()
}

#[cfg(test)]
mod test {
    use crate::{pset_create::validate_address, ElementsNetwork};

    #[test]
    fn test_validate() {
        let testnet_address = "tlq1qq2xvpcvfup5j8zscjq05u2wxxjcyewk7979f3mmz5l7uw5pqmx6xf5xy50hsn6vhkm5euwt72x878eq6zxx2z58hd7zrsg9qn";
        let network = ElementsNetwork::LiquidTestnet;
        let addr = validate_address(testnet_address, network).unwrap();
        assert_eq!(addr.to_string(), testnet_address);

        let network = ElementsNetwork::Liquid;
        assert!(validate_address(testnet_address, network).is_err())
    }
}
