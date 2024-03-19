use std::collections::{HashMap, HashSet};

use elements::{
    pset::{Output, PartiallySignedTransaction},
    Address, Script,
};
use rand::thread_rng;

use crate::{
    model::Recipient, pset_create::IssuanceRequest, ElementsNetwork, Error, UnvalidatedRecipient,
    Wollet, EC,
};

/// A transaction builder
///
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
    addressees: Vec<Recipient>,
    fee_rate: f32,
    issuance_request: IssuanceRequest,
}

impl TxBuilder {
    pub fn new(network: ElementsNetwork) -> Self {
        TxBuilder {
            network,
            addressees: vec![],
            fee_rate: 100.0,
            issuance_request: IssuanceRequest::None,
        }
    }

    pub fn add_recipient(mut self, addr: &UnvalidatedRecipient) -> Result<Self, Error> {
        let addr: Recipient = addr.validate(self.network())?;
        self.addressees.push(addr);
        Ok(self)
    }

    pub fn add_validated_recipient(mut self, addr: Recipient) -> Self {
        self.addressees.push(addr);
        self
    }

    pub fn add_recipients(mut self, addrs: &[UnvalidatedRecipient]) -> Result<Self, Error> {
        for addr in addrs {
            self = self.add_recipient(addr)?;
        }
        Ok(self)
    }

    pub fn add_lbtc_recipient(self, address: &Address, satoshi: u64) -> Result<Self, Error> {
        let rec = UnvalidatedRecipient::lbtc(address.to_string(), satoshi);
        Ok(self.add_recipient(&rec)?)
    }

    pub fn fee_rate(mut self, fee_rate: Option<f32>) -> Self {
        if let Some(fee_rate) = fee_rate {
            self.fee_rate = fee_rate
        }
        self
    }

    pub fn issuance_request(mut self, issuance_request: IssuanceRequest) -> Self {
        self.issuance_request = issuance_request;
        self
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
            .addressees
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
            IssuanceRequest::Reissuance(asset, satoshi_asset, address_asset) => {
                let issuance = wollet.issuance(&asset)?;
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

    fn network(&self) -> &ElementsNetwork {
        &self.network
    }
}
