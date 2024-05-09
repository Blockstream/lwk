use std::collections::{HashMap, HashSet};

use elements::{
    confidential::Value,
    issuance::ContractHash,
    pset::{Output, PartiallySignedTransaction},
    secp256k1_zkp::ZERO_TWEAK,
    Address, AssetId, Script, Transaction,
};
use rand::thread_rng;

use crate::{
    hashes::Hash,
    model::{IssuanceDetails, Recipient},
    pset_create::{validate_address, IssuanceRequest},
    Contract, ElementsNetwork, Error, UnvalidatedRecipient, Wollet, EC,
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

/// A transaction builder
///
/// See [`WolletTxBuilder`] for usage from rust.
///
/// Design decisions:
///
/// * We are not holding a reference of the wallet in the struct and we instead pass a reference
/// of the wallet in the finish methods because this it more friendly for bindings implementation.
/// Moreover, we could have an alternative finish which don't use a wallet at all.
/// * We are consuming and returning self to build the tx with method chaining
#[derive(Debug)]
pub struct TxBuilder {
    network: ElementsNetwork,
    recipients: Vec<Recipient>,
    fee_rate: f32,
    issuance_request: IssuanceRequest,
}

impl TxBuilder {
    /// Creates a transaction builder for bindings code. From rust use [`WolletTxBuilder`]
    pub fn new(network: ElementsNetwork) -> Self {
        TxBuilder {
            network,
            recipients: vec![],
            fee_rate: 100.0,
            issuance_request: IssuanceRequest::None,
        }
    }

    fn network(&self) -> ElementsNetwork {
        self.network
    }

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

    pub fn add_unvalidated_recipient(
        mut self,
        recipient: &UnvalidatedRecipient,
    ) -> Result<Self, Error> {
        let addr: Recipient = recipient.validate(self.network())?;
        self.recipients.push(addr);
        Ok(self)
    }

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

    pub fn add_lbtc_recipient(self, address: &Address, satoshi: u64) -> Result<Self, Error> {
        let rec = UnvalidatedRecipient::lbtc(address.to_string(), satoshi);
        self.add_unvalidated_recipient(&rec)
    }

    pub fn add_burn(self, satoshi: u64, asset_id: AssetId) -> Result<Self, Error> {
        let rec = UnvalidatedRecipient::burn(asset_id.to_string(), satoshi);
        self.add_unvalidated_recipient(&rec)
    }
    /// Fee rate in sats/kvb
    /// Multiply sats/vb value by 1000 i.e. 1.0 sat/byte = 1000.0 sat/kvb
    pub fn fee_rate(mut self, fee_rate: Option<f32>) -> Self {
        if let Some(fee_rate) = fee_rate {
            self.fee_rate = fee_rate
        }
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
        if asset_sats == 0 {
            return Err(Error::InvalidAmount);
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
        self.issuance_request = IssuanceRequest::Reissuance(
            asset_to_reissue,
            satoshi_to_reissue,
            asset_receiver,
            issuance_tx,
        );
        Ok(self)
    }

    pub fn finish(self, wollet: &Wollet) -> Result<PartiallySignedTransaction, Error> {
        // Init PSET
        let mut pset = PartiallySignedTransaction::new_v2();
        let mut inp_txout_sec = HashMap::new();
        let mut last_unused_internal = wollet.change(None)?.index();
        let mut last_unused_external = wollet.address(None)?.index();

        let mut inp_weight = 0;

        let policy_asset = self.network().policy_asset();
        let (addressees_lbtc, addressees_asset): (Vec<_>, Vec<_>) = self
            .recipients
            .into_iter()
            .partition(|a| a.asset == policy_asset);

        // Assets inputs and outputs
        let assets: HashSet<_> = addressees_asset.iter().map(|a| a.asset).collect();
        for asset in assets {
            let mut satoshi_out = 0;
            let mut satoshi_in = 0;
            for addressee in addressees_asset.iter().filter(|a| a.asset == asset) {
                wollet.add_output(&mut pset, addressee)?;
                satoshi_out += addressee.satoshi;
            }
            for utxo in wollet.asset_utxos(&asset)? {
                wollet.add_input(&mut pset, &mut inp_txout_sec, &mut inp_weight, &utxo)?;
                satoshi_in += utxo.unblinded.value;
                if satoshi_in >= satoshi_out {
                    if satoshi_in > satoshi_out {
                        let satoshi_change = satoshi_in - satoshi_out;
                        let addressee = wollet.addressee_change(
                            satoshi_change,
                            asset,
                            &mut last_unused_internal,
                        )?;
                        wollet.add_output(&mut pset, &addressee)?;
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
            wollet.add_output(&mut pset, &addressee)?;
            satoshi_out += addressee.satoshi;
        }

        // FIXME: For implementation simplicity now we always add all L-BTC inputs
        for utxo in wollet.asset_utxos(&wollet.policy_asset())? {
            wollet.add_input(&mut pset, &mut inp_txout_sec, &mut inp_weight, &utxo)?;
            satoshi_in += utxo.unblinded.value;
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

                let addressee = match address_asset {
                    Some(address) => Recipient::from_address(satoshi_asset, &address, asset),
                    None => wollet.addressee_external(
                        satoshi_asset,
                        asset,
                        &mut last_unused_external,
                    )?,
                };
                wollet.add_output(&mut pset, &addressee)?;

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
                            let utxos_token = wollet.asset_utxos(&token)?;
                            let utxo_token = utxos_token
                                .first()
                                .ok_or_else(|| Error::InsufficientFunds)?;
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

        // Add a temporary fee, and always add a change output,
        // then we'll tweak those values to match the given fee rate.
        let temp_fee = 1000;
        if satoshi_in < (satoshi_out + temp_fee) {
            return Err(Error::InsufficientFunds);
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

        let weight = {
            let mut rng = thread_rng();
            let mut temp_pset = pset.clone();
            temp_pset.blind_last(&mut rng, &EC, &inp_txout_sec)?;
            inp_weight + temp_pset.extract_tx()?.weight()
        };

        let vsize = (weight + 4 - 1) / 4;
        let fee = (vsize as f32 * self.fee_rate / 1000.0).ceil() as u64;
        if satoshi_in < (satoshi_out + temp_fee) {
            return Err(Error::InsufficientFunds);
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
        pset.blind_last(&mut rng, &EC, &inp_txout_sec)?;

        // Add details to the pset from our descriptor, like bip32derivation and keyorigin
        wollet.add_details(&mut pset)?;

        Ok(pset)
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

    /// Wrapper of [`TxBuilder::fee_rate()`]
    pub fn fee_rate(self, fee_rate: Option<f32>) -> Self {
        Self {
            wollet: self.wollet,
            inner: self.inner.fee_rate(fee_rate),
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
}
