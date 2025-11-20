use std::collections::{HashMap, HashSet};

use elements::{
    confidential::{AssetBlindingFactor, Nonce, Value, ValueBlindingFactor},
    issuance::ContractHash,
    pset::{raw::ProprietaryKey, Output, PartiallySignedTransaction, PsbtSighashType},
    secp256k1_zkp::{self, RangeProof, SurjectionProof, ZERO_TWEAK},
    Address, AssetId, BlindAssetProofs, BlindValueProofs, EcdsaSighashType, OutPoint, Script,
    Transaction,
};
use rand::thread_rng;

use crate::{
    hashes::Hash,
    liquidex::{self, LiquidexError, Validated},
    model::{ExternalUtxo, IssuanceDetails, Recipient},
    pset_create::{validate_address, IssuanceRequest},
    Contract, ElementsNetwork, Error, LiquidexProposal, UnvalidatedRecipient, Wollet, EC,
};

pub fn extract_issuances(tx: &Transaction) -> Vec<IssuanceDetails> {
    let mut r = vec![];
    for (vin, txin) in tx.input.iter().enumerate() {
        if txin.has_issuance() {
            let contract_hash = ContractHash::from_byte_array(txin.asset_issuance.asset_entropy);
            let entropy = AssetId::generate_asset_entropy(txin.previous_output, contract_hash)
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
    r
}

/// "Clone" of Wollet.add_input
fn add_external_input(
    pset: &mut PartiallySignedTransaction,
    inp_txout_sec: &mut HashMap<usize, elements::TxOutSecrets>,
    inp_weight: &mut usize,
    utxo: &ExternalUtxo,
) -> Result<(), Error> {
    let mut input = elements::pset::Input::from_prevout(utxo.outpoint);
    let mut txout = utxo.txout.clone();
    // This field is used by stateless blinders or signers to
    // learn the blinding factors and unblinded values of this input.
    // We need this since the output witness, which includes the
    // rangeproof, is not serialized.
    // Note that we explicitly remove the txout rangeproof to avoid
    // relying on its presence.
    input.in_utxo_rangeproof = txout.witness.rangeproof.take();
    input.witness_utxo = Some(txout.clone());
    if let Some(tx) = &utxo.tx {
        // For pre-segwit add non_witness_utxo
        let mut tx = tx.clone();
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

    input.asset = Some(utxo.unblinded.asset);
    input.amount = Some(utxo.unblinded.value);
    if let (Some(value_comm), Some(asset_gen)) =
        (txout.value.commitment(), txout.asset.commitment())
    {
        let mut rng = rand::thread_rng();
        input.blind_asset_proof = Some(Box::new(SurjectionProof::blind_asset_proof(
            &mut rng,
            &EC,
            utxo.unblinded.asset,
            utxo.unblinded.asset_bf,
        )?));
        input.blind_value_proof = Some(Box::new(RangeProof::blind_value_proof(
            &mut rng,
            &EC,
            utxo.unblinded.value,
            value_comm,
            asset_gen,
            utxo.unblinded.value_bf,
        )?));
    }

    pset.add_input(input);
    let idx = pset.inputs().len() - 1;
    inp_txout_sec.insert(idx, utxo.unblinded);
    *inp_weight += utxo.max_weight_to_satisfy;
    Ok(())
}

/// A transaction builder
///
/// See [`WolletTxBuilder`] for usage from rust.
///
/// Design decisions:
///
/// * We are not holding a reference of the wallet in the struct and we instead pass a reference
///   of the wallet in the finish methods because this it more friendly for bindings implementation.
///   Moreover, we could have an alternative finish which don't use a wallet at all.
/// * We are consuming and returning self to build the tx with method chaining
#[derive(Debug)]
pub struct TxBuilder {
    network: ElementsNetwork,
    recipients: Vec<Recipient>,
    fee_rate: f32,
    ct_discount: bool,
    issuance_request: IssuanceRequest,
    drain_lbtc: bool,
    drain_to: Option<Address>,
    external_utxos: Vec<ExternalUtxo>,

    selected_utxos: Option<Vec<OutPoint>>,

    // LiquiDEX fields
    is_liquidex_make: bool,
    liquidex_proposals: Vec<LiquidexProposal<Validated>>,
}

impl TxBuilder {
    /// Creates a transaction builder for bindings code. From rust use [`WolletTxBuilder`]
    pub fn new(network: ElementsNetwork) -> Self {
        TxBuilder {
            network,
            recipients: vec![],
            fee_rate: 100.0,
            ct_discount: true,
            issuance_request: IssuanceRequest::None,
            drain_lbtc: false,
            drain_to: None,
            external_utxos: vec![],
            selected_utxos: None,
            is_liquidex_make: false,
            liquidex_proposals: vec![],
        }
    }

    fn network(&self) -> ElementsNetwork {
        self.network
    }

    /// Add recipient to the internal list
    pub fn add_recipient(
        self,
        address: &Address,
        satoshi: u64,
        asset_id: AssetId,
    ) -> Result<Self, Error> {
        let rec = UnvalidatedRecipient {
            satoshi,
            address: address.to_string(),
            asset: asset_id.to_string(),
        };
        self.add_unvalidated_recipient(&rec)
    }

    /// Add unvalidated recipient to the internal list
    pub fn add_unvalidated_recipient(
        mut self,
        recipient: &UnvalidatedRecipient,
    ) -> Result<Self, Error> {
        let addr: Recipient = recipient.validate(self.network())?;
        self.recipients.push(addr);
        Ok(self)
    }

    /// Add validated recipient to the internal list
    pub fn add_validated_recipient(mut self, recipient: Recipient) -> Self {
        self.recipients.push(recipient);
        self
    }

    /// Replace current recipients with the given list
    pub fn set_unvalidated_recipients(
        mut self,
        recipients: &[UnvalidatedRecipient],
    ) -> Result<Self, Error> {
        self.recipients.clear();
        for recipient in recipients {
            self = self.add_unvalidated_recipient(recipient)?;
        }
        Ok(self)
    }

    /// Add L-BTC recipient to the internal list
    pub fn add_lbtc_recipient(self, address: &Address, satoshi: u64) -> Result<Self, Error> {
        let rec = UnvalidatedRecipient::lbtc(address.to_string(), satoshi);
        self.add_unvalidated_recipient(&rec)
    }

    /// Add burn output the internal list
    pub fn add_burn(self, satoshi: u64, asset_id: AssetId) -> Result<Self, Error> {
        let rec = UnvalidatedRecipient::burn(asset_id.to_string(), satoshi);
        self.add_unvalidated_recipient(&rec)
    }

    /// Add explicit output
    pub fn add_explicit_recipient(
        mut self,
        address: &Address,
        satoshi: u64,
        asset: AssetId,
    ) -> Result<Self, Error> {
        if address.blinding_pubkey.is_some() {
            return Err(Error::NotExplicitAddress);
        }
        self.recipients.push(Recipient {
            satoshi,
            script_pubkey: address.script_pubkey(),
            blinding_pubkey: None,
            asset,
        });
        Ok(self)
    }

    /// Fee rate in sats/kvb
    /// Multiply sats/vb value by 1000 i.e. 1.0 sat/byte = 1000.0 sat/kvb
    pub fn fee_rate(mut self, fee_rate: Option<f32>) -> Self {
        if let Some(fee_rate) = fee_rate {
            self.fee_rate = fee_rate
        }
        self
    }

    /// Use ELIP200 discounted fees for Confidential Transactions
    ///
    /// Note: if ELIP200 was not activated by miners and nodes relaying transactions, using
    /// this feature might cause the transaction to be rejected.
    pub fn enable_ct_discount(mut self) -> Self {
        self.ct_discount = true;
        self
    }

    /// Do not use ELIP200 discounted fees for Confidential Transactions
    pub fn disable_ct_discount(mut self) -> Self {
        self.ct_discount = false;
        self
    }

    /// Issue an asset
    ///
    /// There will be `asset_sats` units of this asset that will be received by
    /// `asset_receiver` if it's set, otherwise to an address of the wallet generating the issuance.
    ///
    /// There will be `token_sats` reissuance tokens that allow token holder to reissue the created
    /// asset. Reissuance token will be received by `token_receiver` if it's some, or to an
    /// address of the wallet generating the issuance if none.
    ///
    /// If a `contract` is provided, it's metadata will be committed in the generated asset id.
    ///
    /// Can't be used if `reissue_asset` has been called
    pub fn issue_asset(
        mut self,
        asset_sats: u64,
        asset_receiver: Option<Address>,
        token_sats: u64,
        token_receiver: Option<Address>,
        contract: Option<Contract>,
    ) -> Result<Self, Error> {
        if !matches!(self.issuance_request, IssuanceRequest::None) {
            return Err(Error::IssuanceAlreadySet);
        }
        if let Some(addr) = asset_receiver.as_ref() {
            validate_address(&addr.to_string(), self.network())?;
        }
        if let Some(addr) = token_receiver.as_ref() {
            validate_address(&addr.to_string(), self.network())?;
        }
        if asset_sats == 0 && token_sats == 0 {
            return Err(Error::InvalidAmount);
        }
        if asset_sats > 21_000_000 * 100_000_000 {
            return Err(Error::IssuanceAmountGreaterThanBtcMax);
        }
        self.issuance_request = IssuanceRequest::Issuance(
            asset_sats,
            asset_receiver,
            token_sats,
            token_receiver,
            contract,
        );
        Ok(self)
    }

    /// Reissue an asset
    ///
    /// reissue the asset defined by `asset_to_reissue`, provided the reissuance token is owned
    /// by the wallet generating te reissuance.
    ///
    /// Generated transaction will create `satoshi_to_reissue` new asset units, and they will be
    /// sent to the provided `asset_receiver` address if some, or to an address from the wallet
    /// generating the reissuance transaction if none.
    ///
    /// If the issuance transaction does not involve this wallet,
    /// pass the issuance transaction in `issuance_tx`.
    pub fn reissue_asset(
        mut self,
        asset_to_reissue: AssetId,
        satoshi_to_reissue: u64,
        asset_receiver: Option<Address>,
        issuance_tx: Option<Transaction>,
    ) -> Result<Self, Error> {
        if !matches!(self.issuance_request, IssuanceRequest::None) {
            return Err(Error::IssuanceAlreadySet);
        }
        if let Some(addr) = asset_receiver.as_ref() {
            validate_address(&addr.to_string(), self.network())?;
        }
        if satoshi_to_reissue == 0 {
            return Err(Error::InvalidAmount);
        }
        if satoshi_to_reissue > 21_000_000 * 100_000_000 {
            return Err(Error::IssuanceAmountGreaterThanBtcMax);
        }
        self.issuance_request = IssuanceRequest::Reissuance(
            asset_to_reissue,
            satoshi_to_reissue,
            asset_receiver,
            issuance_tx,
        );
        Ok(self)
    }

    /// Select all available L-BTC inputs
    pub fn drain_lbtc_wallet(mut self) -> Self {
        self.drain_lbtc = true;
        self
    }

    /// Sets the address to drain excess L-BTC to
    pub fn drain_lbtc_to(mut self, address: Address) -> Self {
        self.drain_to = Some(address);
        self
    }

    /// Adds external UTXOs
    ///
    /// Note: unblinded UTXOs with the same scriptpubkeys as the wallet, are considered external.
    pub fn add_external_utxos(mut self, utxos: Vec<ExternalUtxo>) -> Result<Self, Error> {
        self.external_utxos.extend(utxos);
        Ok(self)
    }

    /// Switch to manual coin selection by giving a list of internal UTXOs to use.
    ///
    /// All passed UTXOs are added to the transaction.
    /// No other wallet UTXO is added to the transaction, caller is supposed to add enough UTXOs to
    /// cover for all recipients and fees.
    ///
    /// This method never fails, any error will be raised in [`TxBuilder::finish`].
    ///
    /// Possible errors:
    /// * OutPoint doesn't belong to the wallet
    /// * Insufficient funds (remember to include L-BTC utxos for fees)
    pub fn set_wallet_utxos(mut self, utxos: Vec<OutPoint>) -> Self {
        self.selected_utxos = Some(utxos);
        self
    }

    /// Set data to create a PSET from which you
    /// can create a LiquiDEX proposal
    pub fn liquidex_make(
        mut self,
        utxo: OutPoint,
        address: &Address,
        satoshi: u64,
        asset_id: AssetId,
    ) -> Result<Self, Error> {
        self = self.set_wallet_utxos(vec![utxo]);
        self = self.add_recipient(address, satoshi, asset_id)?;
        self.is_liquidex_make = true;
        Ok(self)
    }

    /// Set data to take LiquiDEX proposals
    pub fn liquidex_take(
        mut self,
        proposals: Vec<LiquidexProposal<Validated>>,
    ) -> Result<Self, Error> {
        self.liquidex_proposals = proposals;
        Ok(self)
    }

    /// Finish building a transaction that can be converted to a LiquiDEX proposal
    fn finish_liquidex_make(
        self,
        wollet: &Wollet,
    ) -> Result<(PartiallySignedTransaction, Vec<String>), Error> {
        // Create PSET
        let mut pset = PartiallySignedTransaction::new_v2();
        let mut inp_txout_sec = HashMap::new();
        let mut inp_weight = 0;

        // Get input outpoint
        let selected_utxos = self
            .selected_utxos
            .ok_or(LiquidexError::MakerInvalidParams)?;
        let &[outpoint] = selected_utxos.as_slice() else {
            return Err(Error::LiquidexError(LiquidexError::MakerInvalidParams));
        };
        // Get output recipient
        let [recipient] = self.recipients.as_slice() else {
            return Err(Error::LiquidexError(LiquidexError::MakerInvalidParams));
        };

        // Add input
        let utxos = wollet.utxos_map()?;
        let utxo = utxos
            .get(&outpoint)
            .ok_or(Error::MissingWalletUtxo(outpoint))?;
        wollet.add_input(&mut pset, &mut inp_txout_sec, &mut inp_weight, utxo)?;

        let input = &mut pset.inputs_mut()[0];
        // Set asset blinding factor
        let txoutsecrets = inp_txout_sec.get(&0).expect("just added");
        input.set_abf(txoutsecrets.asset_bf);
        // Set blind value proof
        let input_scalar_offset = liquidex::scalar_offset(txoutsecrets);
        let blind_value_proof = liquidex::blind_value_proof(txoutsecrets)?;
        input.blind_value_proof = Some(Box::new(blind_value_proof));

        // Set sighash
        input.sighash_type = Some(PsbtSighashType::from_u32(
            EcdsaSighashType::SinglePlusAnyoneCanPay.as_u32(),
        ));

        // Add output
        wollet.add_output(&mut pset, recipient)?;

        // Blind
        let asset = recipient.asset;
        let value = recipient.satoshi;
        let receiver_blinding_pk = recipient
            .blinding_pubkey
            .ok_or(LiquidexError::MakerInvalidParams)?;
        let script_pubkey = &recipient.script_pubkey;
        let mut rng = rand::thread_rng();
        let abf = AssetBlindingFactor::new(&mut rng);
        let vbf = ValueBlindingFactor::new(&mut rng);
        let (nonce, shared_secret) = Nonce::new_confidential(&mut rng, &EC, &receiver_blinding_pk);
        let ecdh_pubkey =
            elements::bitcoin::PublicKey::new(nonce.commitment().expect("confidential"));
        let asset_tag = secp256k1_zkp::Tag::from(asset.into_inner().to_byte_array());
        let asset_generator =
            secp256k1_zkp::Generator::new_blinded(&EC, asset_tag, abf.into_inner());
        let value_commitment =
            secp256k1_zkp::PedersenCommitment::new(&EC, value, vbf.into_inner(), asset_generator);
        let min_value = if script_pubkey.is_provably_unspendable() {
            0
        } else {
            1
        };

        fn make_rangeproof_message(asset: AssetId, bf: secp256k1_zkp::Tweak) -> [u8; 64] {
            let mut message = [0u8; 64];

            message[..32].copy_from_slice(&asset.into_inner().to_byte_array());
            message[32..].copy_from_slice(bf.as_ref());

            message
        }

        let message = make_rangeproof_message(asset, abf.into_inner());

        let rangeproof = secp256k1_zkp::RangeProof::new(
            &EC,
            min_value,
            value_commitment,
            value,
            vbf.into_inner(),
            &message,
            script_pubkey.as_bytes(),
            shared_secret,
            0,
            52,
            asset_generator,
        )?;

        let output = &mut pset.outputs_mut()[0];
        output.asset_comm = Some(asset_generator);
        output.amount_comm = Some(value_commitment);
        output.ecdh_pubkey = Some(ecdh_pubkey);
        output.value_rangeproof = Some(Box::new(rangeproof));
        // We need to set an asset surjection proof, otherwise rust-elements does not serialize the PSET
        // https://github.com/ElementsProject/rust-elements/blob/master/src/pset/map/output.rs#L581
        let bytes = [
            1, 0, 1, 69, 162, 31, 81, 9, 102, 83, 180, 22, 237, 171, 76, 161, 122, 220, 124, 208,
            90, 74, 148, 162, 247, 161, 89, 3, 139, 112, 101, 185, 126, 78, 3, 188, 6, 32, 154,
            164, 175, 175, 158, 239, 225, 188, 83, 222, 42, 159, 10, 155, 216, 114, 78, 89, 163,
            124, 134, 74, 83, 104, 116, 254, 137, 218, 19,
        ];
        let fake_surjectionproof =
            secp256k1_zkp::SurjectionProof::from_slice(&bytes).expect("hardcoded");
        output.asset_surjection_proof = Some(Box::new(fake_surjectionproof));
        output.set_abf(abf);
        let txoutsecrets = elements::TxOutSecrets {
            asset,
            asset_bf: abf,
            value,
            value_bf: vbf,
        };
        let output_scalar_offset = liquidex::scalar_offset(&txoutsecrets);
        let blind_value_proof = liquidex::blind_value_proof(&txoutsecrets)?;
        output.blind_value_proof = Some(Box::new(blind_value_proof));
        // Add blind asset proof
        // Note: this is technically redundant since one could use the asset and abf,
        // but we add it to make things easier for verifiers,
        // so they don't have to handle the liquidex special case.
        let blind_asset_proof =
            secp256k1_zkp::SurjectionProof::blind_asset_proof(&mut rng, &EC, asset, abf)?;
        output.blind_asset_proof = Some(Box::new(blind_asset_proof));

        // Add scalar
        // Compute the scalar offset to be added to the last vbf by the Taker to balance the transaction:
        // abf_i * value_i + vbf_i - (abf_o * value_o + vbf_o)
        let mut tweak = ValueBlindingFactor::from_slice(input_scalar_offset.as_ref())?;
        tweak += -ValueBlindingFactor::from_slice(output_scalar_offset.as_ref())?;
        pset.global.scalars = vec![tweak.into_inner()];

        // Add details to the pset from our descriptor, like bip32derivation and keyorigin
        wollet.add_details(&mut pset)?;

        // TODO: blinding nonces
        Ok((pset, vec![]))
    }

    /// Finish building a transaction that takes LiquiDEX proposals
    fn finish_liquidex_take(
        self,
        wollet: &Wollet,
    ) -> Result<(PartiallySignedTransaction, Vec<String>), Error> {
        let [proposal] = self.liquidex_proposals.as_slice() else {
            return Err(Error::LiquidexError(LiquidexError::TakerInvalidParams));
        };

        // Create PSET
        let mut pset = proposal.to_pset()?;
        let mut inp_txout_sec = HashMap::new();
        let mut inp_weight = 0;
        let mut input_domain = vec![];
        let mut last_unused_internal = wollet.change(None)?.index();
        let mut last_unused_external = wollet.address(None)?.index();
        let mut rng = thread_rng();

        let utxos = wollet.utxos_map()?;

        let [input] = pset.inputs() else {
            return Err(Error::LiquidexError(LiquidexError::TakerInvalidParams));
        };
        let maker_input_asset = input.asset.ok_or(LiquidexError::TakerInvalidParams)?;
        let maker_input_satoshi = input.amount.ok_or(LiquidexError::TakerInvalidParams)?;
        let maker_input_abf = input.get_abf().ok_or(LiquidexError::TakerInvalidParams)??;
        let [output] = pset.outputs() else {
            return Err(Error::LiquidexError(LiquidexError::TakerInvalidParams));
        };
        let maker_output_asset = output.asset.ok_or(LiquidexError::TakerInvalidParams)?;
        let maker_output_satoshi = output.amount.ok_or(LiquidexError::TakerInvalidParams)?;
        let maker_output_abf = output
            .get_abf()
            .ok_or(LiquidexError::TakerInvalidParams)??;

        // Maker input
        let surj_input = elements::SurjectionInput::Known {
            asset: maker_input_asset,
            asset_bf: maker_input_abf,
        };
        input_domain.push(surj_input.surjection_target(&EC).expect("known"));
        // In general the maker input is the only input with its asset, thus so need to pass its
        // asset and abf to "blind_last" so that it can create the surjection proof for the outputs
        // with its asset.
        // However the only way to pass these data to "blind_last" is through a TxOutSecrets in the
        // inp_txout_sec map, but we don't have the maker input value blinding factor (vbf).
        // Therefore we choose a vbf that has a *zero* scalar offset, so that is does not affect the
        // last vbf computation and rangeproof creation (its contribution to the last vbf is
        // already in the scalars field). Nevertheless when creating the surjection proofs for
        // outputs which asset is the same as the maker input, it can access the asset and abf from
        // inp_txout_sec.
        // vbf = - abf * v;
        let value_bf =
            ValueBlindingFactor::last(&EC, maker_input_satoshi, maker_input_abf, &[], &[]);
        let maker_input_txout_sec = elements::TxOutSecrets {
            asset: maker_input_asset,
            asset_bf: maker_input_abf,
            value: maker_input_satoshi,
            value_bf,
        };
        let idx = 0;
        inp_txout_sec.insert(idx, maker_input_txout_sec);

        // Add taker output (from proposal)
        let addressee = wollet.addressee_external(
            maker_input_satoshi,
            maker_input_asset,
            &mut last_unused_external,
        )?;
        wollet.add_output(&mut pset, &addressee)?;

        // Add inputs and change for maker output (if not L-BTC)
        // FIXME: If the wallet is taking a proposal made by the wallet itself, do not add the "maker" input again.
        if maker_output_asset != wollet.policy_asset() {
            let satoshi_out = maker_output_satoshi;
            let mut satoshi_in = 0;
            for utxo in utxos
                .values()
                .filter(|u| u.unblinded.asset == maker_output_asset)
            {
                wollet.add_input(&mut pset, &mut inp_txout_sec, &mut inp_weight, utxo)?;
                let surj_input = elements::SurjectionInput::from_txout_secrets(utxo.unblinded);
                input_domain.push(surj_input.surjection_target(&EC).expect("from secrets"));
                satoshi_in += utxo.unblinded.value;
                if satoshi_in >= satoshi_out {
                    if satoshi_in > satoshi_out {
                        let satoshi_change = satoshi_in - satoshi_out;
                        let addressee = wollet.addressee_change(
                            satoshi_change,
                            maker_output_asset,
                            &mut last_unused_internal,
                        )?;
                        wollet.add_output(&mut pset, &addressee)?;
                    }
                    break;
                }
            }
            if satoshi_in < satoshi_out {
                return Err(Error::InsufficientFunds {
                    missing_sats: satoshi_out - satoshi_in,
                    asset_id: maker_output_asset,
                    is_token: false,
                });
            }
        }

        // Add inputs, change and fees for L-BTC
        let mut satoshi_out = 0;
        let mut satoshi_in = 0;
        if maker_output_asset == wollet.policy_asset() {
            satoshi_out += maker_output_satoshi;
        }
        if maker_input_asset == wollet.policy_asset() {
            satoshi_in += maker_input_satoshi;
            // We added the taker output above
            satoshi_out += maker_input_satoshi;
        }

        // FIXME: For implementation simplicity now we always add all L-BTC inputs
        for utxo in utxos
            .values()
            .filter(|u| u.unblinded.asset == wollet.policy_asset())
        {
            wollet.add_input(&mut pset, &mut inp_txout_sec, &mut inp_weight, utxo)?;
            let surj_input = elements::SurjectionInput::from_txout_secrets(utxo.unblinded);
            input_domain.push(surj_input.surjection_target(&EC).expect("from secrets"));
            satoshi_in += utxo.unblinded.value;
        }

        // Add a temporary fee, and always add a change or drain output,
        // then we'll tweak those values to match the given fee rate.
        let temp_fee = 1;
        if satoshi_in <= (satoshi_out + temp_fee) {
            return Err(Error::InsufficientFunds {
                missing_sats: (satoshi_out + temp_fee + 1) - satoshi_in, // +1 to ensure we have more than just equal
                asset_id: wollet.policy_asset(),
                is_token: false,
            });
        }
        let satoshi_change = satoshi_in - satoshi_out - temp_fee;
        let addressee = wollet.addressee_change(
            satoshi_change,
            wollet.policy_asset(),
            &mut last_unused_internal,
        )?;
        wollet.add_output(&mut pset, &addressee)?;
        let fee_output =
            Output::new_explicit(Script::default(), temp_fee, wollet.policy_asset(), None);
        pset.add_output(fee_output);

        for (vout, output) in pset.outputs_mut().iter_mut().enumerate() {
            // For the maker output, create the surjection proof
            if vout == 0 {
                let asset_tag =
                    secp256k1_zkp::Tag::from(maker_output_asset.into_inner().to_byte_array());

                let surjectionproof = secp256k1_zkp::SurjectionProof::new(
                    &EC,
                    &mut rng,
                    asset_tag,
                    maker_output_abf.into_inner(),
                    &input_domain,
                )?;

                output.asset_surjection_proof = Some(Box::new(surjectionproof));
                output.blinder_index = None;
            }
            // Set all blinder index to 1 except for the maker output (1st) and the fee
            if (vout > 0) && !output.script_pubkey.is_empty() {
                output.blinder_index = Some(1);
            }
        }

        let weight = {
            let mut temp_pset = pset.clone();
            temp_pset.blind_last(&mut rng, &EC, &inp_txout_sec)?;
            let tx_weight = {
                let tx = temp_pset.extract_tx()?;
                if self.ct_discount {
                    tx.discount_weight()
                } else {
                    tx.weight()
                }
            };
            inp_weight + tx_weight
        };

        let vsize = weight.div_ceil(4);
        let fee = (vsize as f32 * self.fee_rate / 1000.0).ceil() as u64;
        if satoshi_in <= (satoshi_out + fee) {
            return Err(Error::InsufficientFunds {
                missing_sats: (satoshi_out + fee + 1) - satoshi_in, // +1 to ensure we have more than just equal
                asset_id: wollet.policy_asset(),
                is_token: false,
            });
        }
        let satoshi_change = satoshi_in - satoshi_out - fee;
        // Replace change and fee outputs
        let n_outputs = pset.n_outputs();
        let outputs = pset.outputs_mut();
        let change_output = &mut outputs[n_outputs - 2]; // index check: we always have the lbtc change and the fee output at least
        change_output.amount = Some(satoshi_change);
        let fee_output = &mut outputs[n_outputs - 1];
        fee_output.amount = Some(fee);

        // Blind the transaction
        pset.blind_last(&mut rng, &EC, &inp_txout_sec)?;

        // Add details to the pset from our descriptor, like bip32derivation and keyorigin
        wollet.add_details(&mut pset)?;

        // TODO: blinding nonces
        Ok((pset, vec![]))
    }

    /// Finish building the transaction for AMP0
    #[cfg(feature = "amp0")]
    pub fn finish_for_amp0(self, wollet: &Wollet) -> Result<crate::amp0::Amp0Pset, Error> {
        let (pset, blinding_nonces) = self.finish_inner(wollet, true)?;
        crate::amp0::Amp0Pset::new(pset, blinding_nonces)
    }

    /// Finish building the transaction
    pub fn finish(self, wollet: &Wollet) -> Result<PartiallySignedTransaction, Error> {
        let (pset, _blinding_nonces) = self.finish_inner(wollet, false)?;
        Ok(pset)
    }

    fn finish_inner(
        self,
        wollet: &Wollet,
        for_amp0: bool,
    ) -> Result<(PartiallySignedTransaction, Vec<String>), Error> {
        if self.is_liquidex_make {
            return self.finish_liquidex_make(wollet);
        } else if !self.liquidex_proposals.is_empty() {
            return self.finish_liquidex_take(wollet);
        }
        // Init PSET
        let mut pset = PartiallySignedTransaction::new_v2();

        let genesis_block_hash = self.network().genesis_block_hash().to_byte_array().to_vec();
        // Add genesis block hash as defined in ELIP-101 https://github.com/ElementsProject/ELIPs/blob/main/elip-0101.mediawiki
        // TODO: upstream to rust elements
        // TODO: tested with Jade 1.0.37 but does not work. Safe to merge because subtype is unique.
        const PSBT_ELEMENTS_GLOBAL_GENESIS_HASH: u8 = 0x02;
        pset.global.proprietary.insert(
            ProprietaryKey::from_pset_pair(PSBT_ELEMENTS_GLOBAL_GENESIS_HASH, vec![]),
            genesis_block_hash,
        );
        let mut inp_txout_sec = HashMap::new();
        let mut last_unused_internal = wollet.last_unused_internal();
        let mut last_unused_external = wollet.last_unused_external();

        let mut inp_weight = 0;

        let utxos = wollet.utxos_map()?;

        let policy_asset = self.network().policy_asset();
        let (addressees_lbtc, addressees_asset): (Vec<_>, Vec<_>) = self
            .recipients
            .into_iter()
            .partition(|a| a.asset == policy_asset);

        // Get selected utxos (manual coin selection)
        let mut selected_utxos = vec![];
        if let Some(ref coins) = self.selected_utxos {
            for coin in coins {
                let utxo = utxos.get(coin).ok_or(Error::MissingWalletUtxo(*coin))?;
                selected_utxos.push(utxo);
            }
        }

        // Assets that belongs to this transaction
        // all the ones with a recipient
        let mut assets: HashSet<_> = addressees_asset.iter().map(|a| a.asset).collect();
        // and all the ones of utxos that are being added
        for utxo in &self.external_utxos {
            assets.insert(utxo.unblinded.asset);
        }
        for utxo in &selected_utxos {
            assets.insert(utxo.unblinded.asset);
        }
        // Policy asset is handled separately below
        assets.remove(&policy_asset);

        for asset in assets {
            let mut satoshi_out = 0;
            let mut satoshi_in = 0;
            for addressee in addressees_asset.iter().filter(|a| a.asset == asset) {
                wollet.add_output(&mut pset, addressee)?;
                satoshi_out += addressee.satoshi;
            }

            // Add all external asset utxos
            for utxo in &self.external_utxos {
                if utxo.unblinded.asset != asset {
                    continue;
                }
                add_external_input(&mut pset, &mut inp_txout_sec, &mut inp_weight, utxo)?;
                satoshi_in += utxo.unblinded.value;
            }

            if self.selected_utxos.is_some() {
                // Add only selected asset utxos
                for utxo in &selected_utxos {
                    if utxo.unblinded.asset != asset {
                        continue;
                    }
                    wollet.add_input(&mut pset, &mut inp_txout_sec, &mut inp_weight, utxo)?;
                    satoshi_in += utxo.unblinded.value;
                }
            } else {
                // Add more asset utxos until we cover the amount to send
                if satoshi_in < satoshi_out {
                    for utxo in utxos.values().filter(|u| u.unblinded.asset == asset) {
                        wollet.add_input(&mut pset, &mut inp_txout_sec, &mut inp_weight, utxo)?;
                        satoshi_in += utxo.unblinded.value;
                        if satoshi_in >= satoshi_out {
                            break;
                        }
                    }
                }
            }

            // Add change
            if satoshi_in > satoshi_out {
                let satoshi_change = satoshi_in - satoshi_out;
                let addressee =
                    wollet.addressee_change(satoshi_change, asset, &mut last_unused_internal)?;
                wollet.add_output(&mut pset, &addressee)?;
            }

            // Insufficient funds
            if satoshi_in < satoshi_out {
                return Err(Error::InsufficientFunds {
                    missing_sats: satoshi_out - satoshi_in,
                    asset_id: asset,
                    is_token: false,
                });
            }
        }

        // L-BTC inputs and outputs
        // Fee and L-BTC change after (re)issuance
        let mut satoshi_out = 0;
        let mut satoshi_in = 0;
        for addressee in addressees_lbtc {
            wollet.add_output(&mut pset, &addressee)?;
            satoshi_out += addressee.satoshi;
        }

        // Add all external L-BTC utxos
        for utxo in &self.external_utxos {
            if utxo.unblinded.asset != policy_asset {
                continue;
            }
            add_external_input(&mut pset, &mut inp_txout_sec, &mut inp_weight, utxo)?;
            satoshi_in += utxo.unblinded.value;
        }

        if self.selected_utxos.is_some() {
            for utxo in &selected_utxos {
                if utxo.unblinded.asset != policy_asset {
                    continue;
                }
                wollet.add_input(&mut pset, &mut inp_txout_sec, &mut inp_weight, utxo)?;
                satoshi_in += utxo.unblinded.value;
            }
        } else {
            // FIXME: For implementation simplicity now we always add all L-BTC inputs
            for utxo in utxos.values().filter(|u| u.unblinded.asset == policy_asset) {
                wollet.add_input(&mut pset, &mut inp_txout_sec, &mut inp_weight, utxo)?;
                satoshi_in += utxo.unblinded.value;
            }
        }

        // Set (re)issuance data
        match self.issuance_request {
            IssuanceRequest::None => {}
            IssuanceRequest::Issuance(
                satoshi_asset,
                address_asset,
                satoshi_token,
                address_token,
                contract,
            ) => {
                // At least a L-BTC input for the fee was added.
                let idx = 0;
                let (asset, token) =
                    wollet.set_issuance(&mut pset, idx, satoshi_asset, satoshi_token, contract)?;

                if satoshi_asset > 0 {
                    let addressee = match address_asset {
                        Some(address) => Recipient::from_address(satoshi_asset, &address, asset),
                        None => wollet.addressee_external(
                            satoshi_asset,
                            asset,
                            &mut last_unused_external,
                        )?,
                    };
                    wollet.add_output(&mut pset, &addressee)?;
                }

                if satoshi_token > 0 {
                    let addressee = match address_token {
                        Some(address) => Recipient::from_address(satoshi_token, &address, token),
                        None => wollet.addressee_external(
                            satoshi_token,
                            token,
                            &mut last_unused_external,
                        )?,
                    };
                    wollet.add_output(&mut pset, &addressee)?;
                }
            }
            IssuanceRequest::Reissuance(asset, satoshi_asset, address_asset, issuance_tx) => {
                let issuance = if let Some(issuance_tx) = issuance_tx {
                    extract_issuances(&issuance_tx)
                        .iter()
                        .find(|i| i.asset == asset)
                        .ok_or_else(|| Error::MissingIssuance)?
                        .clone()
                } else {
                    wollet.issuance(&asset)?
                };
                let token = issuance.token;
                // Find or add input for the token
                let (idx, token_asset_bf) =
                    match inp_txout_sec.iter().find(|(_, u)| u.asset == token) {
                        Some((idx, u)) => (*idx, u.asset_bf),
                        None => {
                            // Add an input sending the token,
                            let utxos_token: Vec<_> = utxos
                                .values()
                                .filter(|u| u.unblinded.asset == token)
                                .collect();
                            let utxo_token =
                                utxos_token
                                    .first()
                                    .ok_or_else(|| Error::InsufficientFunds {
                                        missing_sats: 1, // We need at least one token
                                        asset_id: token,
                                        is_token: true,
                                    })?;
                            let idx = wollet.add_input(
                                &mut pset,
                                &mut inp_txout_sec,
                                &mut inp_weight,
                                utxo_token,
                            )?;

                            // and an outpout receiving the token
                            let satoshi_token = utxo_token.unblinded.value;
                            let addressee = wollet.addressee_change(
                                satoshi_token,
                                token,
                                &mut last_unused_internal,
                            )?;
                            wollet.add_output(&mut pset, &addressee)?;

                            (idx, utxo_token.unblinded.asset_bf)
                        }
                    };

                // Set reissuance data
                wollet.set_reissuance(
                    &mut pset,
                    idx,
                    satoshi_asset,
                    &token_asset_bf,
                    &issuance.entropy,
                )?;

                let addressee = match address_asset {
                    Some(address) => Recipient::from_address(satoshi_asset, &address, asset),
                    None => wollet.addressee_external(
                        satoshi_asset,
                        asset,
                        &mut last_unused_external,
                    )?,
                };
                wollet.add_output(&mut pset, &addressee)?;
            }
        }

        // Add a temporary fee, and always add a change or drain output,
        // then we'll tweak those values to match the given fee rate.
        let temp_fee = 1;
        if satoshi_in <= (satoshi_out + temp_fee) {
            return Err(Error::InsufficientFunds {
                missing_sats: (satoshi_out + temp_fee + 1) - satoshi_in, // +1 to ensure we have more than just equal
                asset_id: wollet.policy_asset(),
                is_token: false,
            });
        }
        let satoshi_change = satoshi_in - satoshi_out - temp_fee;
        let addressee = if let Some(address) = self.drain_to {
            Recipient::from_address(satoshi_change, &address, wollet.policy_asset())
        } else {
            wollet.addressee_change(
                satoshi_change,
                wollet.policy_asset(),
                &mut last_unused_internal,
            )?
        };
        wollet.add_output(&mut pset, &addressee)?;
        let fee_output =
            Output::new_explicit(Script::default(), temp_fee, wollet.policy_asset(), None);
        pset.add_output(fee_output);

        let weight = {
            let mut rng = thread_rng();
            let mut temp_pset = pset.clone();
            temp_pset.blind_last(&mut rng, &EC, &inp_txout_sec)?;
            let tx_weight = {
                let tx = temp_pset.extract_tx()?;
                if self.ct_discount {
                    tx.discount_weight()
                } else {
                    tx.weight()
                }
            };
            inp_weight + tx_weight
        };

        let vsize = weight.div_ceil(4);
        let fee = (vsize as f32 * self.fee_rate / 1000.0).ceil() as u64;
        if satoshi_in <= (satoshi_out + fee) {
            return Err(Error::InsufficientFunds {
                missing_sats: (satoshi_out + fee + 1) - satoshi_in, // +1 to ensure we have more than just equal
                asset_id: wollet.policy_asset(),
                is_token: false,
            });
        }
        let satoshi_change = satoshi_in - satoshi_out - fee;
        // Replace change and fee outputs
        let n_outputs = pset.n_outputs();
        let outputs = pset.outputs_mut();
        let change_output = &mut outputs[n_outputs - 2]; // index check: we always have the lbtc change and the fee output at least
        change_output.amount = Some(satoshi_change);
        let fee_output = &mut outputs[n_outputs - 1];
        fee_output.amount = Some(fee);

        // TODO inputs/outputs(except fee) randomization, not trivial because of blinder_index on inputs

        // Blind the transaction
        let mut rng = thread_rng();

        // TODO: use the next line once we can use elements26 only
        // let blind_secrets = pset.blind_last(&mut rng, &EC, &inp_txout_sec)?;
        let (blind_secrets, mut pset) = if for_amp0 {
            use elements26::confidential::{
                AssetBlindingFactor as Abf26, ValueBlindingFactor as Vbf26,
            };
            use elements26::pset::PartiallySignedTransaction as Pset26;
            use std::str::FromStr;
            let mut pset26 = Pset26::from_str(&pset.to_string()).expect("from elements25");
            let inp_txout_sec: HashMap<usize, elements26::TxOutSecrets> = inp_txout_sec
                .iter()
                .map(|(i, s)| {
                    let asset = elements26::AssetId::from_slice(s.asset.into_inner().as_ref())
                        .expect("from elements25");
                    let abf = Abf26::from_slice(s.asset_bf.into_inner().as_ref())
                        .expect("from elements25");
                    let vbf = Vbf26::from_slice(s.value_bf.into_inner().as_ref())
                        .expect("from elements25");
                    let value = s.value;
                    let s = elements26::TxOutSecrets::new(asset, abf, value, vbf);
                    (*i, s)
                })
                .collect();
            let blind_secrets = pset26
                .blind_last(&mut rng, &EC, &inp_txout_sec)
                .map_err(|e| Error::Generic(format!("elements26 blind error: {e}")))?;
            // erase all non witness utxo surjection and range proofs
            // this appears to be necessary for pre-segwit inputs
            for input in pset26.inputs_mut() {
                if let Some(ref mut tx) = &mut input.non_witness_utxo {
                    for output in &mut tx.output {
                        output.witness = Default::default();
                    }
                }
            }
            let pset25 = elements::pset::PartiallySignedTransaction::from_str(&pset26.to_string())
                .expect("from elements25");
            (blind_secrets, pset25)
        } else {
            let blind_secrets = std::collections::BTreeMap::new();
            pset.blind_last(&mut rng, &EC, &inp_txout_sec)?;
            (blind_secrets, pset)
        };

        let mut m = HashMap::new();
        for (ct_location, (_abf, _vbf, eph_sk)) in blind_secrets.iter() {
            // these are outputs not inputs...
            if let elements26::CtLocation {
                input_index,
                ty: elements26::CtLocationType::Input,
            } = ct_location
            {
                m.insert(input_index, eph_sk);
            }
        }

        let mut blinding_nonces = vec![];
        for idx in 0..pset.n_outputs() {
            let bn = if let Some(eph_sk) = m.get(&idx) {
                let blinding_pubkey = pset.outputs()[idx]
                    .blinding_key
                    .ok_or_else(|| Error::Generic("Missing blinding key".into()))?;
                let (_nonce, shared_secret) = elements::confidential::Nonce::with_ephemeral_sk(
                    &EC,
                    **eph_sk,
                    &blinding_pubkey.inner,
                );
                shared_secret.display_secret().to_string()
            } else {
                "".to_string()
            };
            blinding_nonces.push(bn);
        }

        // Add details to the pset from our descriptor, like bip32derivation and keyorigin
        wollet.add_details(&mut pset)?;

        Ok((pset, blinding_nonces))
    }
}

/// A transaction builder.
#[derive(Debug)]
pub struct WolletTxBuilder<'a> {
    wollet: &'a Wollet,
    inner: TxBuilder,
}

impl<'a> WolletTxBuilder<'a> {
    /// Creates a transaction builder. Could be conveniently created with [`Wollet::tx_builder()`]
    pub fn new(wollet: &'a Wollet) -> Self {
        WolletTxBuilder {
            wollet,
            inner: TxBuilder::new(wollet.network()),
        }
    }

    /// Consume this builder and create a transaction
    pub fn finish(self) -> Result<PartiallySignedTransaction, Error> {
        self.inner.finish(self.wollet)
    }

    /// Consume this builder and create a transaction for AMP0
    #[cfg(feature = "amp0")]
    pub fn finish_for_amp0(self) -> Result<crate::amp0::Amp0Pset, Error> {
        self.inner.finish_for_amp0(self.wollet)
    }

    /// Wrapper of [`TxBuilder::add_recipient()`]
    pub fn add_recipient(
        self,
        address: &Address,
        satoshi: u64,
        asset_id: AssetId,
    ) -> Result<Self, Error> {
        Ok(Self {
            wollet: self.wollet,
            inner: self.inner.add_recipient(address, satoshi, asset_id)?,
        })
    }

    /// Wrapper of [`TxBuilder::add_unvalidated_recipient()`]
    pub fn add_unvalidated_recipient(
        self,
        recipient: &UnvalidatedRecipient,
    ) -> Result<Self, Error> {
        Ok(Self {
            wollet: self.wollet,
            inner: self.inner.add_unvalidated_recipient(recipient)?,
        })
    }

    /// Wrapper of [`TxBuilder::add_validated_recipient()`]
    pub fn add_validated_recipient(self, recipient: Recipient) -> Self {
        Self {
            wollet: self.wollet,
            inner: self.inner.add_validated_recipient(recipient),
        }
    }

    /// Wrapper of [`TxBuilder::set_unvalidated_recipients()`]
    pub fn set_unvalidated_recipients(
        self,
        recipients: &[UnvalidatedRecipient],
    ) -> Result<Self, Error> {
        Ok(Self {
            wollet: self.wollet,
            inner: self.inner.set_unvalidated_recipients(recipients)?,
        })
    }

    /// Wrapper of [`TxBuilder::add_lbtc_recipient()`]
    pub fn add_lbtc_recipient(self, address: &Address, satoshi: u64) -> Result<Self, Error> {
        Ok(Self {
            wollet: self.wollet,
            inner: self.inner.add_lbtc_recipient(address, satoshi)?,
        })
    }

    /// Wrapper of [`TxBuilder::add_burn()`]
    pub fn add_burn(self, satoshi: u64, asset_id: AssetId) -> Result<Self, Error> {
        Ok(Self {
            wollet: self.wollet,
            inner: self.inner.add_burn(satoshi, asset_id)?,
        })
    }

    /// Wrapper of [`TxBuilder::add_explicit_recipient()`]
    pub fn add_explicit_recipient(
        self,
        address: &Address,
        satoshi: u64,
        asset_id: AssetId,
    ) -> Result<Self, Error> {
        Ok(Self {
            wollet: self.wollet,
            inner: self
                .inner
                .add_explicit_recipient(address, satoshi, asset_id)?,
        })
    }

    /// Wrapper of [`TxBuilder::fee_rate()`]
    pub fn fee_rate(self, fee_rate: Option<f32>) -> Self {
        Self {
            wollet: self.wollet,
            inner: self.inner.fee_rate(fee_rate),
        }
    }

    /// Wrapper of [`TxBuilder::enable_ct_discount()`]
    pub fn enable_ct_discount(self) -> Self {
        Self {
            wollet: self.wollet,
            inner: self.inner.enable_ct_discount(),
        }
    }

    /// Wrapper of [`TxBuilder::disable_ct_discount()`]
    pub fn disable_ct_discount(self) -> Self {
        Self {
            wollet: self.wollet,
            inner: self.inner.disable_ct_discount(),
        }
    }

    /// Wrapper of [`TxBuilder::issue_asset()`]
    pub fn issue_asset(
        self,
        asset_sats: u64,
        asset_receiver: Option<Address>,
        token_sats: u64,
        token_receiver: Option<Address>,
        contract: Option<Contract>,
    ) -> Result<Self, Error> {
        Ok(Self {
            wollet: self.wollet,
            inner: self.inner.issue_asset(
                asset_sats,
                asset_receiver,
                token_sats,
                token_receiver,
                contract,
            )?,
        })
    }

    /// Wrapper of [`TxBuilder::reissue_asset()`]
    pub fn reissue_asset(
        self,
        asset_to_reissue: AssetId,
        satoshi_to_reissue: u64,
        asset_receiver: Option<Address>,
        issuance_tx: Option<Transaction>,
    ) -> Result<Self, Error> {
        Ok(Self {
            wollet: self.wollet,
            inner: self.inner.reissue_asset(
                asset_to_reissue,
                satoshi_to_reissue,
                asset_receiver,
                issuance_tx,
            )?,
        })
    }

    /// Wrapper of [`TxBuilder::drain_lbtc_wallet()`]
    pub fn drain_lbtc_wallet(self) -> Self {
        Self {
            wollet: self.wollet,
            inner: self.inner.drain_lbtc_wallet(),
        }
    }

    /// Wrapper of [`TxBuilder::drain_lbtc_to()`]
    pub fn drain_lbtc_to(self, address: Address) -> Self {
        Self {
            wollet: self.wollet,
            inner: self.inner.drain_lbtc_to(address),
        }
    }

    /// Wrapper of [`TxBuilder::add_external_utxos()`]
    pub fn add_external_utxos(self, utxos: Vec<ExternalUtxo>) -> Result<Self, Error> {
        Ok(Self {
            wollet: self.wollet,
            inner: self.inner.add_external_utxos(utxos)?,
        })
    }

    /// Wrapper of [`TxBuilder::set_wallet_utxos()`]
    pub fn set_wallet_utxos(self, utxos: Vec<OutPoint>) -> Self {
        Self {
            wollet: self.wollet,
            inner: self.inner.set_wallet_utxos(utxos),
        }
    }

    /// Wrapper of [`TxBuilder::liquidex_make()`]
    pub fn liquidex_make(
        self,
        utxo: OutPoint,
        address: &Address,
        satoshi: u64,
        asset_id: AssetId,
    ) -> Result<Self, Error> {
        Ok(Self {
            wollet: self.wollet,
            inner: self.inner.liquidex_make(utxo, address, satoshi, asset_id)?,
        })
    }

    /// Wrapper of [`TxBuilder::liquidex_take()`]
    pub fn liquidex_take(self, proposals: Vec<LiquidexProposal<Validated>>) -> Result<Self, Error> {
        Ok(Self {
            wollet: self.wollet,
            inner: self.inner.liquidex_take(proposals)?,
        })
    }
}

#[cfg(test)]
mod tests {
    use elements::encode::Decodable;

    use super::*;

    #[test]
    fn test_extract_issuances() {
        let tx_bytes = include_bytes!(
            "../tests/data/62ea5d0aa7c9f4339b16a6d8e6ff4437ffb244de658222841c74d335324e4219"
        );
        let tx = Transaction::consensus_decode(&tx_bytes[..]).unwrap();
        let issuances = extract_issuances(&tx);
        assert_eq!(issuances.len(), 1);
        let issuance = &issuances[0];
        assert_eq!(
            issuance.txid.to_string(),
            "62ea5d0aa7c9f4339b16a6d8e6ff4437ffb244de658222841c74d335324e4219"
        );
        assert_eq!(issuance.vin, 0);
        assert_eq!(
            issuance.asset.to_string(),
            "71aba14535beded7753ba1f3a0ff3d47a166363fa06a27eb65559abc92b4bc09"
        );
        assert_eq!(
            issuance.token.to_string(),
            "82cd33501102795d04a9eb093bcfd5434da9d993e40cef5ba7c5a8fa1750bf8f"
        );
        assert!(!issuance.is_reissuance);
        assert_eq!(issuance.asset_amount, Some(1000000000));
        assert_eq!(issuance.token_amount, Some(1));
    }

    #[test]
    fn test_serialize_proprietary_key() {
        use elements::encode::Encodable;
        use elements::hex::ToHex;
        let key = elements::pset::raw::ProprietaryKey::from_pset_pair(0x02, vec![]);
        let mut buf = vec![];
        key.consensus_encode(&mut buf).unwrap();
        assert_eq!(buf, vec![4, 112, 115, 101, 116, 2]);
        let pair = elements::pset::raw::Pair {
            key: key.to_key(),
            value: ElementsNetwork::Liquid
                .genesis_block_hash()
                .to_byte_array()
                .to_vec(),
        };
        let mut buf = vec![];
        pair.consensus_encode(&mut buf).unwrap();
        assert_eq!(
            buf.to_hex(),
            "07fc047073657402200360208a889692372c8d68b084a62efdf60ea1a359a04c94b20d223658276614",
        );
    }
}
