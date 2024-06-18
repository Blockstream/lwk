use crate::bitcoin::PublicKey as BitcoinPublicKey;
use crate::elements::confidential::AssetBlindingFactor;
use crate::elements::issuance::ContractHash;
use crate::elements::pset::{Input, Output, PartiallySignedTransaction};
use crate::elements::{Address, AssetId, OutPoint, Transaction, TxOut, TxOutSecrets, Txid};
use crate::error::Error;
use crate::hashes::Hash;
use crate::model::{Recipient, WalletTxOut};
use crate::registry::Contract;
use crate::wollet::Wollet;
use crate::ElementsNetwork;
use elements::pset::elip100::AssetMetadata;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Serialize, Deserialize)]
// We make issuance and reissuance are mutually exclusive for simplicity
pub enum IssuanceRequest {
    None,
    Issuance(u64, Option<Address>, u64, Option<Address>, Option<Contract>),
    Reissuance(AssetId, u64, Option<Address>, Option<Transaction>),
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
        let value_comm = txout.value.commitment().expect("TODO");
        let asset_gen = txout.asset.commitment().expect("TODO");
        // This field is used by stateless blinders or signers to
        // learn the blinding factors and unblinded values of this input.
        // We need this since the output witness, which includes the
        // rangeproof, is not serialized.
        // Note that we explicitly remove the txout rangeproof to avoid
        // relying on its presence.
        input.in_utxo_rangeproof = txout.witness.rangeproof.take();
        input.witness_utxo = Some(txout);
        // Needed by ledger
        let mut rng = rand::thread_rng();
        let secp = elements::secp256k1_zkp::Secp256k1::new();
        use elements::secp256k1_zkp::{RangeProof, SurjectionProof};
        use elements::{BlindAssetProofs, BlindValueProofs};

        input.asset = Some(utxo.unblinded.asset);
        input.blind_asset_proof = Some(Box::new(
            SurjectionProof::blind_asset_proof(
                &mut rng,
                &secp,
                utxo.unblinded.asset,
                utxo.unblinded.asset_bf,
            )
            .expect("TODO"),
        ));
        input.amount = Some(utxo.unblinded.value);
        input.blind_value_proof = Some(Box::new(
            RangeProof::blind_value_proof(
                &mut rng,
                &secp,
                utxo.unblinded.value,
                value_comm,
                asset_gen,
                utxo.unblinded.value_bf,
            )
            .expect("TODO"),
        ));

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
        input.blinded_issuance = Some(0x00);

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
        input.blinded_issuance = Some(0x00);
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
