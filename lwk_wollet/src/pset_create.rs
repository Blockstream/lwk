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
use crate::{ElementsNetwork, EC};
use elements::pset::elip100::{AssetMetadata, TokenMetadata};
use std::collections::HashMap;

pub const SECP256K1_SURJECTIONPROOF_MAX_N_INPUTS: usize = 256;

#[derive(Debug)]
// We make issuance and reissuance are mutually exclusive for simplicity
pub enum IssuanceRequest {
    None,
    Issuance(u64, Option<Address>, u64, Option<Address>, Option<Contract>),
    Reissuance(AssetId, u64, Option<Address>, Option<Transaction>),
}

impl Wollet {
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
        if pset.inputs().len() >= SECP256K1_SURJECTIONPROOF_MAX_N_INPUTS {
            return Err(Error::TooManyInputs(pset.inputs().len()));
        }

        let mut input = Input::from_prevout(utxo.outpoint);
        let mut txout = self.get_txout(&utxo.outpoint)?;
        let (Some(value_comm), Some(asset_gen)) =
            (txout.value.commitment(), txout.asset.commitment())
        else {
            return Err(Error::NotConfidentialInput);
        };
        // This field is used by stateless blinders or signers to
        // learn the blinding factors and unblinded values of this input.
        // We need this since the output witness, which includes the
        // rangeproof, is not serialized.
        // Note that we explicitly remove the txout rangeproof to avoid
        // relying on its presence.
        input.in_utxo_rangeproof = txout.witness.rangeproof.take();
        input.witness_utxo = Some(txout);

        if !self.is_segwit() {
            // For pre-segwit add non_witness_utxo
            let mut tx = self.get_tx(&utxo.outpoint.txid)?;
            // Remove the rangeproof to match the witness utxo,
            // to pass the checks done by elements-miniscript
            let _ = tx
                .output
                .get_mut(utxo.outpoint.vout as usize)
                .expect("got txout above")
                .witness
                .rangeproof
                .take();
            input.non_witness_utxo = Some(tx);
        }

        // Needed by ledger
        let mut rng = rand::thread_rng();
        let secp = &EC;
        use elements::secp256k1_zkp::{RangeProof, SurjectionProof};
        use elements::{BlindAssetProofs, BlindValueProofs};

        input.asset = Some(utxo.unblinded.asset);
        input.blind_asset_proof = Some(Box::new(SurjectionProof::blind_asset_proof(
            &mut rng,
            secp,
            utxo.unblinded.asset,
            utxo.unblinded.asset_bf,
        )?));
        input.amount = Some(utxo.unblinded.value);
        input.blind_value_proof = Some(Box::new(RangeProof::blind_value_proof(
            &mut rng,
            secp,
            utxo.unblinded.value,
            value_comm,
            asset_gen,
            utxo.unblinded.value_bf,
        )?));

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
        if satoshi_asset > 0 {
            input.issuance_value_amount = Some(satoshi_asset);
        }
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
            // TODO: handle blinded issuance
            let issuance_blinded = false;
            pset.add_token_metadata(token, &TokenMetadata::new(asset, issuance_blinded));
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

    fn addressee_inner(
        &self,
        satoshi: u64,
        asset: AssetId,
        last_unused: &mut u32,
        is_change: bool,
    ) -> Result<Recipient, Error> {
        #[cfg(feature = "amp0")]
        if self.descriptor.is_amp0() {
            // For AMP0 we never want to use addresses that are not monitored by the server.
            // We never want to use:
            // * index 0
            // * indexes greater than "last_index" returned by the server
            //
            // GDK at login uploads 20 addresses for AMP0 accounts, so as long as we're in
            // the [1,20] range we're fine.
            //
            // This is quite conservative and might cause some address reuse, but it ensures
            // that the tx builder does not use addresses that are not monitored by the server.
            *last_unused = (*last_unused).clamp(1, 20);

            let params = self.network().address_params();
            let address = self.descriptor.amp0_address(*last_unused, params)?;
            *last_unused += 1;
            return Ok(Recipient::from_address(satoshi, &address, asset));
        }

        let address = if is_change {
            self.change(Some(*last_unused))?
        } else {
            self.address(Some(*last_unused))?
        };
        *last_unused += 1;
        Ok(Recipient::from_address(satoshi, address.address(), asset))
    }

    pub(crate) fn addressee_change(
        &self,
        satoshi: u64,
        asset: AssetId,
        last_unused: &mut u32,
    ) -> Result<Recipient, Error> {
        self.addressee_inner(satoshi, asset, last_unused, true)
    }

    pub(crate) fn addressee_external(
        &self,
        satoshi: u64,
        asset: AssetId,
        last_unused: &mut u32,
    ) -> Result<Recipient, Error> {
        self.addressee_inner(satoshi, asset, last_unused, false)
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
    use crate::{pset_create::validate_address, ElementsNetwork, Update, WolletDescriptor};

    use super::*;
    use crate::NoPersist;

    #[test]
    fn test_validate() {
        let testnet_address = "tlq1qq2xvpcvfup5j8zscjq05u2wxxjcyewk7979f3mmz5l7uw5pqmx6xf5xy50hsn6vhkm5euwt72x878eq6zxx2z58hd7zrsg9qn";
        let network = ElementsNetwork::LiquidTestnet;
        let addr = validate_address(testnet_address, network).unwrap();
        assert_eq!(addr.to_string(), testnet_address);

        let network = ElementsNetwork::Liquid;
        assert!(validate_address(testnet_address, network).is_err())
    }

    #[test]
    fn test_add_input_exceeds_limit() {
        let wollet = test_wollet_with_many_transactions();

        let mut pset = PartiallySignedTransaction::default();
        let mut inp_txout_sec = HashMap::new();
        let mut inp_weight = 0usize;
        let dummy_utxo = wollet.utxos().unwrap()[0].clone();
        for _ in 0..SECP256K1_SURJECTIONPROOF_MAX_N_INPUTS {
            let res = wollet.add_input(&mut pset, &mut inp_txout_sec, &mut inp_weight, &dummy_utxo);
            assert!(res.is_ok());
        }
        let result = wollet.add_input(&mut pset, &mut inp_txout_sec, &mut inp_weight, &dummy_utxo);

        match result {
            Err(Error::TooManyInputs(count)) => {
                assert_eq!(count, SECP256K1_SURJECTIONPROOF_MAX_N_INPUTS);
            }
            _ => panic!("Expected TooManyInputs error"),
        }
    }

    // duplicated from tests/test_wollet.rs
    pub fn test_wollet_with_many_transactions() -> Wollet {
        let update = lwk_test_util::update_test_vector_many_transactions();
        let descriptor = lwk_test_util::wollet_descriptor_many_transactions();
        let descriptor: WolletDescriptor = descriptor.parse().unwrap();
        let update = Update::deserialize(&update).unwrap();
        let mut wollet = Wollet::new(
            ElementsNetwork::LiquidTestnet,
            std::sync::Arc::new(NoPersist {}),
            descriptor,
        )
        .unwrap();
        wollet.apply_update(update).unwrap();
        wollet
    }
}
