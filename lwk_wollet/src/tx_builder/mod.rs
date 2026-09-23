use std::{collections::HashMap, vec};

use crate::{
    issuance::{IssuanceOutput, IssuanceRequest, Issuances, ReissuanceRequest, Reissuances},
    liquidex::Validated,
    model::{ExternalUtxo, IssuanceDetails, Recipient},
    pegin::PeginInput,
    pset_create::{
        validate_address, validate_address_explicit, SECP256K1_SURJECTIONPROOF_MAX_N_INPUTS,
    },
    Contract, Error, LiquidexProposal, Network, UnvalidatedRecipient, Wollet, EC,
};
use elements::{
    confidential::Value, pset::PartiallySignedTransaction, Address, AssetBlindingNonce, AssetId,
    OutPoint, RangeProof, Script, SurjectionProof, Transaction, TxOut, TxOutSecrets,
};
use lwk_common::FeeRate;
use rand::thread_rng;

mod built_tx;
mod finish;
mod liquidex;
mod wollet_tx_builder;

pub use built_tx::BuiltTx;
pub use wollet_tx_builder::WolletTxBuilder;

/// Validates the outputs receiving `total` (re)issued satoshi, filling in the default single
/// output to the wallet generating the (re)issuance if the caller added none
fn validate_outputs(
    outputs: &mut Vec<IssuanceOutput>,
    total: u64,
    network: Network,
) -> Result<(), Error> {
    if outputs.is_empty() {
        if total > 0 {
            outputs.push(IssuanceOutput {
                satoshi: total,
                address: None,
            });
        }
        return Ok(());
    }

    let mut sum: u64 = 0;
    for output in outputs.iter() {
        if output.satoshi == 0 {
            return Err(Error::InvalidAmount);
        }
        if let Some(address) = output.address.as_ref() {
            validate_address(address, network)?;
        }
        sum = sum
            .checked_add(output.satoshi)
            .ok_or(Error::InvalidAmount)?;
    }

    if sum != total {
        return Err(Error::IssuanceOutputsAmountMismatch {
            expected: total,
            found: sum,
        });
    }

    Ok(())
}

pub fn extract_issuances(tx: &Transaction) -> Vec<IssuanceDetails> {
    let mut r = vec![];
    for (vin, txin) in tx.input.iter().enumerate() {
        if txin.has_issuance() {
            let contract_hash = txin.asset_issuance.asset_entropy.into_contract_hash();
            let entropy = AssetId::generate_asset_entropy(txin.previous_output, contract_hash)
                .to_byte_array();
            let (asset, token) = txin.issuance_ids();
            let is_reissuance =
                txin.asset_issuance.asset_blinding_nonce != AssetBlindingNonce::NEW_ISSUANCE;
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
    add_input_rangeproofs: bool,
) -> Result<usize, Error> {
    add_input_inner(
        pset,
        inp_txout_sec,
        inp_weight,
        utxo.outpoint,
        utxo.txout.clone(),
        utxo.tx.clone(),
        utxo.unblinded,
        utxo.max_weight_to_satisfy,
        true, // external inputs can be explicit
        add_input_rangeproofs,
    )
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn add_input_inner(
    pset: &mut PartiallySignedTransaction,
    inp_txout_sec: &mut HashMap<usize, TxOutSecrets>,
    inp_weight: &mut usize,
    outpoint: OutPoint,
    mut txout: TxOut,
    tx: Option<Transaction>,
    unblinded: TxOutSecrets,
    max_weight_to_satisfy: usize,
    allow_explicit_input: bool,
    add_input_rangeproofs: bool,
) -> Result<usize, Error> {
    if pset.inputs().len() >= SECP256K1_SURJECTIONPROOF_MAX_N_INPUTS {
        return Err(Error::TooManyInputs(pset.inputs().len()));
    }

    let mut input = elements::pset::Input::from_prevout(outpoint);

    input.asset = Some(unblinded.asset);
    input.amount = Some(unblinded.value);
    if let (Some(value_comm), Some(asset_gen)) =
        (txout.value.commitment(), txout.asset.commitment())
    {
        // Add the blind proofs
        // These are used to prove that the asset and amount field commit to
        // the asset and value/amount commitment of the output that is being
        // spent.
        let mut rng = rand::thread_rng();
        input.blind_asset_proof = Some(SurjectionProof::blind_asset_proof(
            &mut rng,
            &EC,
            unblinded.asset,
            unblinded.asset_bf,
        )?);
        input.blind_value_proof = Some(RangeProof::blind_value_proof(
            &mut rng,
            &EC,
            unblinded.value,
            value_comm,
            asset_gen,
            unblinded.value_bf,
        )?);
    } else if !allow_explicit_input {
        return Err(Error::NotConfidentialInput);
    }

    // Note that we explicitly remove the txout rangeproof to avoid
    // relying on its presence.
    let rangeproof = std::mem::replace(&mut txout.witness.rangeproof, RangeProof::EMPTY);
    if add_input_rangeproofs && !rangeproof.is_empty() {
        // This field is used by _some_ stateless blinders or signers to
        // learn the blinding factors and unblinded values of this input.
        // We need this since the output witness, which includes the
        // rangeproof, is not serialized.
        input.in_utxo_rangeproof = Some(rangeproof);
    }
    input.witness_utxo = Some(txout.clone());

    if let Some(mut tx) = tx {
        // For pre-segwit add non_witness_utxo
        // Remove the rangeproof to match the witness utxo,
        // to pass the checks done by elements-miniscript
        tx.output
            .get_mut(outpoint.vout as usize)
            .expect("got txout above")
            .witness
            .rangeproof = RangeProof::EMPTY;
        input.non_witness_utxo = Some(tx);
    }

    pset.add_input(input);
    let idx = pset.inputs().len() - 1;
    inp_txout_sec.insert(idx, unblinded);
    *inp_weight += max_weight_to_satisfy;
    Ok(idx)
}

/// Weight of the transaction that the given pset will produce
fn measure_weight(
    pset: &PartiallySignedTransaction,
    inp_txout_sec: &HashMap<usize, TxOutSecrets>,
    inp_weight: usize,
    ct_discount: bool,
) -> Result<usize, Error> {
    let mut rng = thread_rng();
    let mut temp_pset = pset.clone();
    if needs_blinding(&temp_pset)? {
        temp_pset.blind_last(&mut rng, &EC, inp_txout_sec)?;
    }
    let tx = temp_pset.extract_tx()?;
    let tx_weight = if ct_discount {
        tx.discount_weight()
    } else {
        tx.weight()
    };
    Ok(inp_weight + tx_weight)
}

/// Whether the PSET has outputs to blind, erroring if it has confidential inputs
/// but no blinded outputs were provided
fn needs_blinding(pset: &PartiallySignedTransaction) -> Result<bool, Error> {
    if pset.outputs().iter().any(|o| o.blinding_key.is_some()) {
        return Ok(true);
    }
    // checking the witness utxo is enough, since this pset is created by finish_inner(),
    // and add_input_inner() always sets it
    let confidential_input = pset.inputs().iter().any(|i| {
        i.witness_utxo
            .as_ref()
            .is_some_and(|u| u.is_partially_blinded())
    });
    if confidential_input {
        return Err(Error::MissingBlindedOutput);
    }
    Ok(false)
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
    network: Network,
    recipients: Vec<Recipient>,
    fee_rate: FeeRate,
    allow_high_fee_rate: bool,
    ct_discount: bool,
    reissuances: Reissuances,
    issuances: Issuances,
    drain_lbtc: bool,
    drain_to: Option<Address>,
    external_utxos: Vec<ExternalUtxo>,
    pegin_inputs: Vec<PeginInput>,

    selected_utxos: Option<Vec<OutPoint>>,
    inputs_order: Option<Vec<OutPoint>>,

    add_input_rangeproofs: bool,

    // LiquiDEX fields
    is_liquidex_make: bool,
    liquidex_proposals: Vec<LiquidexProposal<Validated>>,
}
impl TxBuilder {
    /// Maximum fee rate in sats/kvb (1000 sat/vb), see [`TxBuilder::allow_high_fee_rate()`]
    pub const MAX_FEE_RATE: f32 = 1_000_000.0;

    /// Creates a transaction builder for bindings code. From rust use [`WolletTxBuilder`]
    pub fn new(network: Network) -> Self {
        TxBuilder {
            network,
            recipients: vec![],
            fee_rate: FeeRate::default(),
            allow_high_fee_rate: false,
            ct_discount: true,
            reissuances: Reissuances::default(),
            issuances: Issuances::None,
            drain_lbtc: false,
            drain_to: None,
            external_utxos: vec![],
            pegin_inputs: vec![],
            selected_utxos: None,
            inputs_order: None,
            add_input_rangeproofs: true,
            is_liquidex_make: false,
            liquidex_proposals: vec![],
        }
    }

    fn network(&self) -> Network {
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

    /// Add an `OP_RETURN` output carrying arbitrary `data`, with zero value.
    pub fn add_op_return(mut self, data: &[u8]) -> Result<Self, Error> {
        self.recipients.push(Recipient {
            satoshi: 0,
            script_pubkey: Script::new_op_return(data),
            blinding_pubkey: None,
            asset: *self.network().policy_asset(),
        });
        Ok(self)
    }

    /// Add explicit output
    pub fn add_explicit_recipient(
        mut self,
        address: &Address,
        satoshi: u64,
        asset: AssetId,
    ) -> Result<Self, Error> {
        validate_address_explicit(address, self.network())?;
        self.recipients.push(Recipient {
            satoshi,
            script_pubkey: address.script_pubkey(),
            blinding_pubkey: None,
            asset,
        });
        Ok(self)
    }

    /// Fee rate, see [`FeeRate`]
    pub fn fee_rate(mut self, fee_rate: Option<FeeRate>) -> Self {
        if let Some(fee_rate) = fee_rate {
            self.fee_rate = fee_rate
        }
        self
    }

    /// Allow a fee rate higher than [`TxBuilder::MAX_FEE_RATE`]
    pub fn allow_high_fee_rate(mut self) -> Self {
        self.allow_high_fee_rate = true;
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

    /// Construct the PSET adding the input rangeproofs
    ///
    /// Default: `true`.
    ///
    /// To verify the asset and amount/value of an input, you can use either
    /// the blind proofs or the rangeproof.
    ///
    /// LWK now always adds the blind proofs, so the rangeproofs are technically
    /// redudant. However old versions of LWK did not add the blind proofs and
    /// only added only the input rangeproofs.
    /// Consistently, the old LWK verification code ignored the blind proofs
    /// and only looked at the input rangeproofs.
    ///
    /// So for backward compatibility purposes, now LWK verification code looks
    /// for both blind proofs and rangeproofs, and when constructing a PSET LWK
    /// adds both blind proofs and rangeproofs.
    ///
    /// This is not ideal since rangeproofs are large and redundant, but you can
    /// skip adding the input rangeproofs passing `false` to this function.
    ///
    /// By passing `true` you can preserve the addition of input rangeproofs.
    ///
    /// After a grace period, we plan to switch the default to `false`.
    pub fn add_input_rangeproofs(mut self, add_rangeproofs: bool) -> Self {
        self.add_input_rangeproofs = add_rangeproofs;
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
    /// Can't be used if [`TxBuilder::reissue_asset`] has been called
    pub fn issue_asset(
        self,
        asset_sats: u64,
        asset_receiver: Option<Address>,
        token_sats: u64,
        token_receiver: Option<Address>,
        contract: Option<Contract>,
    ) -> Result<Self, Error> {
        let mut request = IssuanceRequest::new(asset_sats, token_sats);

        if asset_sats > 0 {
            request = request.add_asset_output(asset_sats, asset_receiver);
        }
        if token_sats > 0 {
            request = request.add_token_output(token_sats, token_receiver);
        }
        if let Some(c) = contract {
            request = request.contract(c);
        }

        self.add_issuance(request)
    }

    /// Issue an asset, or several assets by calling this multiple times in the same transaction
    ///
    /// **Experimental**: this API might change without notice.
    ///
    /// `request` sets the asset/token amounts, receivers, contract and pinning; see
    /// [`IssuanceRequest`] for details.
    ///
    /// Can't be used if [`TxBuilder::reissue_asset`] has been called
    pub fn add_issuance(mut self, mut request: IssuanceRequest) -> Result<Self, Error> {
        if !self.reissuances.requests.is_empty() {
            return Err(Error::IssuanceReissuanceMutuallyExclusive);
        }

        if request.satoshi_asset == 0 && request.satoshi_token == 0 {
            return Err(Error::InvalidAmount);
        }

        validate_outputs(
            &mut request.asset_outputs,
            request.satoshi_asset,
            self.network,
        )?;
        validate_outputs(
            &mut request.token_outputs,
            request.satoshi_token,
            self.network,
        )?;

        match (&mut self.issuances, request.pinned_input) {
            (Issuances::None, Some(input)) => {
                self.issuances = Issuances::Pinned(vec![(request, input)])
            }
            (Issuances::None, None) => self.issuances = Issuances::Sequential(vec![request]),
            (Issuances::Pinned(requests), Some(input)) => requests.push((request, input)),
            (Issuances::Sequential(requests), None) => requests.push(request),
            _ => return Err(Error::IssuanceModesMixing),
        }

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
    ///
    /// Can't be used if [`TxBuilder::issue_asset`] or [`TxBuilder::add_issuance`] has been called
    pub fn reissue_asset(
        self,
        asset_to_reissue: AssetId,
        satoshi_to_reissue: u64,
        asset_receiver: Option<Address>,
        issuance_tx: Option<Transaction>,
    ) -> Result<Self, Error> {
        let mut reissuance_request = ReissuanceRequest::new(asset_to_reissue, satoshi_to_reissue)
            .add_asset_output(satoshi_to_reissue, asset_receiver);

        if let Some(tx) = issuance_tx {
            reissuance_request = reissuance_request.issuance_tx(tx);
        }

        self.add_reissuance(reissuance_request)
    }

    /// Reissue an asset, or several assets by calling this multiple times in the same transaction
    ///
    /// **Experimental**: this API might change without notice.
    ///
    /// `request` sets the asset to reissue, amount, receivers, and issuance transaction; see
    /// [`ReissuanceRequest`] for details.
    ///
    /// Can be called multiple times to reissue several assets in the same transaction, as long
    /// as each call targets a different asset.
    ///
    /// Can't be used if [`TxBuilder::issue_asset`] or [`TxBuilder::add_issuance`] has been called
    pub fn add_reissuance(
        mut self,
        mut reissuance_request: ReissuanceRequest,
    ) -> Result<Self, Error> {
        if !matches!(self.issuances, Issuances::None) {
            return Err(Error::IssuanceReissuanceMutuallyExclusive);
        }

        if reissuance_request.satoshi_to_reissue == 0 {
            return Err(Error::InvalidAmount);
        }

        validate_outputs(
            &mut reissuance_request.asset_outputs,
            reissuance_request.satoshi_to_reissue,
            self.network(),
        )?;

        self.reissuances.add(reissuance_request)?;

        Ok(self)
    }

    /// Select all available L-BTC inputs
    pub fn drain_lbtc_wallet(mut self) -> Self {
        self.drain_lbtc = true;
        self
    }

    /// Sets the address to drain excess L-BTC to
    pub fn drain_lbtc_to(mut self, address: &Address) -> Result<Self, Error> {
        validate_address(address, self.network())?;
        self.drain_to = Some(address.clone());
        Ok(self)
    }

    /// Sets the (explicit, non-confidential) address to drain excess L-BTC to
    pub fn drain_lbtc_to_explicit(mut self, address: &Address) -> Result<Self, Error> {
        validate_address_explicit(address, self.network())?;
        self.drain_to = Some(address.clone());
        Ok(self)
    }

    /// Adds external UTXOs
    ///
    /// Note: unblinded UTXOs with the same scriptpubkeys as the wallet, are considered external.
    pub fn add_external_utxos(mut self, utxos: Vec<ExternalUtxo>) -> Result<Self, Error> {
        self.external_utxos.extend(utxos);
        Ok(self)
    }

    /// Add an authenticated pegin as an L-BTC input.
    ///
    /// Pegin inputs cannot currently be combined with manual input ordering or
    /// LiquiDEX transactions.
    ///
    /// # Errors
    ///
    /// Returns an error if the pegin belongs to another network, if its
    /// outpoint has already been added, or if it cannot be represented as a
    /// PSET input.
    pub fn add_pegin_input(mut self, input: PeginInput) -> Result<Self, Error> {
        let fed_peg_network = input
            .funding()
            .deposit()
            .pegin_address()
            .fed_peg()
            .network();
        if self.network() != fed_peg_network {
            return Err(Error::PeginNetworkMismatch {
                wallet: self.network().as_str(),
                fed_peg: fed_peg_network.as_str(),
            });
        }

        let pset_input = input.to_pset_input()?;
        let outpoint = OutPoint::new(pset_input.previous_txid, pset_input.previous_output_index);
        let bitcoin_outpoint = input.funding().deposit().outpoint();
        if self
            .pegin_inputs
            .iter()
            .any(|candidate| candidate.funding().deposit().outpoint() == bitcoin_outpoint)
        {
            return Err(Error::DuplicatedOutpoint(
                outpoint,
                "pegin inputs".to_string(),
            ));
        }

        self.pegin_inputs.push(input);
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

    /// Orders inputs selected with manual coin selection and external utxos
    ///
    /// **Experimental**: this API might change without notice.
    ///
    /// If this is called, [`TxBuilder::set_wallet_utxos`] must be called too.
    /// The outpoints passed to this method, must be the union of the outpoints passed to [`TxBuilder::set_wallet_utxos`] and [`TxBuilder::add_external_utxos`].
    pub fn set_inputs_order(mut self, inputs_order: Vec<OutPoint>) -> Self {
        self.inputs_order = Some(inputs_order);
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

    /// Finish building the transaction for AMP0
    #[cfg(feature = "amp0")]
    pub fn finish_for_amp0(self, wollet: &Wollet) -> Result<crate::amp0::Amp0Pset, Error> {
        let built_tx = self.finish_inner(wollet)?;
        built_tx.to_amp0()
    }

    /// Finish building the transaction
    pub fn finish(self, wollet: &Wollet) -> Result<PartiallySignedTransaction, Error> {
        let BuiltTx { pset, .. } = self.finish_inner(wollet)?;
        Ok(pset)
    }

    /// Build the transaction
    ///
    /// **Experimental**: this API might change without notice.
    ///
    /// Alternative to `finish()` that returns more than a PSET.
    pub fn build(self, wollet: &Wollet) -> Result<BuiltTx, Error> {
        self.finish_inner(wollet)
    }
}

#[cfg(test)]
mod tests {
    use elements::encode::Decodable;

    use super::*;
    use crate::{
        clients::LastUnused, update::default_blockheader, DownloadTxResult, Update, WolletBuilder,
    };
    use std::collections::BTreeMap;

    #[test]
    fn test_extract_issuances() {
        let tx_bytes = include_bytes!(
            "../../tests/data/62ea5d0aa7c9f4339b16a6d8e6ff4437ffb244de658222841c74d335324e4219"
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
        use elements::hex::DisplayHex;
        let key = elements::pset::raw::ProprietaryKey::from_pset_pair(0x02, vec![]);
        let mut buf = vec![];
        key.consensus_encode(&mut buf).unwrap();
        assert_eq!(buf, vec![4, 112, 115, 101, 116, 2]);
        let pair = elements::pset::raw::Pair {
            key: key.to_key(),
            value: Network::Liquid.genesis_hash().to_byte_array().to_vec(),
        };
        let mut buf = vec![];
        pair.consensus_encode(&mut buf).unwrap();
        assert_eq!(
            buf.to_lower_hex_string(),
            "07fc047073657402200360208a889692372c8d68b084a62efdf60ea1a359a04c94b20d223658276614",
        );
    }

    #[test]
    fn test_builttx_update_keeps_wollet_last_unused() {
        let desc: crate::WolletDescriptor =
            lwk_test_util::wollet_descriptor_string().parse().unwrap();
        let mut wollet = WolletBuilder::new(Network::default_regtest(), desc)
            .build()
            .unwrap();

        wollet
            .apply_update(Update {
                version: 4,
                wollet_status: wollet.status(),
                new_txs: DownloadTxResult::default(),
                txid_height_new: vec![],
                txid_height_delete: vec![],
                timestamps: vec![],
                scripts_with_blinding_pubkey: vec![],
                tip: default_blockheader(),
                unspent: vec![],
                last_unused: LastUnused {
                    external: 7,
                    internal: 11,
                },
            })
            .unwrap();

        let built = BuiltTx {
            pset: PartiallySignedTransaction::new_v2(),
            blind_secrets: BTreeMap::new(),
        };

        let update = built.update(&wollet).unwrap();

        assert_eq!(update.last_unused.external, 7);
        assert_eq!(update.last_unused.internal, 11);
    }

    #[test]
    fn test_high_fee_rate() {
        let desc: crate::WolletDescriptor =
            lwk_test_util::wollet_descriptor_string().parse().unwrap();
        let wollet = WolletBuilder::new(Network::default_regtest(), desc)
            .build()
            .unwrap();
        let address = wollet.address(None).unwrap().address().clone();
        let builder = || {
            let rate = FeeRate::from_sat_kvb(TxBuilder::MAX_FEE_RATE * 2.0).unwrap();
            wollet
                .tx_builder()
                .add_lbtc_recipient(&address, 1000)
                .unwrap()
                .fee_rate(Some(rate))
        };

        let err = builder().finish().unwrap_err();
        assert!(matches!(err, Error::FeeRateTooHigh(_)), "{err:?}");

        // The empty wallet fails later
        let err = builder().allow_high_fee_rate().finish().unwrap_err();
        assert!(matches!(err, Error::InsufficientFunds { .. }), "{err:?}");
    }
}
