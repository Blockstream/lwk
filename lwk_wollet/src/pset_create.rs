use crate::bitcoin::PublicKey as BitcoinPublicKey;
use crate::contract::Contract;
use crate::elements::confidential::AssetBlindingFactor;
use crate::elements::issuance::ContractHash;
use crate::elements::pset::{Output, PartiallySignedTransaction};
use crate::elements::{Address, AssetId, OutPoint, Transaction, TxOut, TxOutSecrets, Txid};
use crate::error::Error;
use crate::hashes::Hash;
use crate::model::{Recipient, WalletTxOut};
use crate::tx_builder::add_input_inner;
use crate::wollet::Wollet;
use crate::ElementsNetwork;
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
        add_input_rangeproofs: bool,
    ) -> Result<usize, Error> {
        let tx = if self.is_segwit() {
            None
        } else {
            // For pre-segwit we want to add non_witness_utxo
            Some(self.get_tx(&utxo.outpoint.txid)?)
        };

        add_input_inner(
            pset,
            inp_txout_sec,
            inp_weight,
            utxo.outpoint,
            self.get_txout(&utxo.outpoint)?,
            tx,
            utxo.unblinded,
            self.max_weight_to_satisfy(),
            false, // wallet inputs cannot be explicit
            add_input_rangeproofs,
        )
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

        let index = if self.descriptor.has_wildcard() {
            Some(*last_unused)
        } else if self.descriptor.spk_count().is_some() {
            // For wollets without descriptor, for now we do a safe choice and internally always use the first scriptpubkey
            Some(0)
        } else {
            // Descriptor wollet with no wildcard
            None
        };
        let address = if is_change {
            self.change(index)?
        } else {
            self.address(index)?
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
            let res = wollet.add_input(
                &mut pset,
                &mut inp_txout_sec,
                &mut inp_weight,
                &dummy_utxo,
                false,
            );
            assert!(res.is_ok());
        }
        let result = wollet.add_input(
            &mut pset,
            &mut inp_txout_sec,
            &mut inp_weight,
            &dummy_utxo,
            false,
        );

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
        let mut wollet =
            Wollet::without_persist(ElementsNetwork::LiquidTestnet, descriptor).unwrap();
        wollet.apply_update(update).unwrap();
        wollet
    }
}
