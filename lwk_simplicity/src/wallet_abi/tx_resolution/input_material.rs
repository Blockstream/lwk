use crate::error::WalletAbiError;
use crate::wallet_abi::schema::{
    AmountFilter, AssetFilter, InputSchema, InputUnblinding, LockFilter, UTXOSource,
    WalletProviderMeta, WalletSourceFilter,
};
use crate::wallet_abi::tx_resolution::resolver::Resolver;
use crate::wallet_abi::tx_resolution::supply_and_demand::{CandidateScore, SupplyAndDemand};

use std::collections::HashSet;
use std::marker::PhantomData;

use lwk_wollet::elements::confidential::{Asset, AssetBlindingFactor, Value, ValueBlindingFactor};
use lwk_wollet::elements::{OutPoint, TxOut};
use lwk_wollet::ExternalUtxo;

use simplicityhl::elements::TxOutSecrets;

pub(crate) struct InputMaterialResolver<'a, WalletProvider: WalletProviderMeta> {
    used_outpoints: HashSet<OutPoint>,
    resolver: &'a Resolver<'a, WalletProvider>,
    _wallet_provider: PhantomData<WalletProvider>,
}

pub(crate) struct ResolvedInputMaterial {
    outpoint: OutPoint,
    tx_out: TxOut,
    secrets: TxOutSecrets,
    wallet_finalization_weight: Option<usize>,
}

impl ResolvedInputMaterial {
    /// Expose the resolved prevout because later stages need one canonical
    /// outpoint for PSET wiring and issuance-id derivation.
    pub(crate) fn outpoint(&self) -> &OutPoint {
        &self.outpoint
    }

    /// Expose the resolved prevout output so the PSET carries the exact witness
    /// UTXO being spent.
    pub(crate) fn tx_out(&self) -> &TxOut {
        &self.tx_out
    }

    /// Expose unblinded secrets so balancing, blinding, and issuance logic all
    /// consume the same resolved source of truth.
    pub(crate) fn secrets(&self) -> &TxOutSecrets {
        &self.secrets
    }

    /// Expose modeled wallet finalization weight so fee estimation can account
    /// for wallet-owned inputs before signatures exist.
    pub(crate) fn wallet_finalization_weight(&self) -> &Option<usize> {
        &self.wallet_finalization_weight
    }
}

impl<'a, WalletProvider: WalletProviderMeta> InputMaterialResolver<'a, WalletProvider>
where
    WalletAbiError: From<WalletProvider::Error>,
{
    /// Create a resolver that tracks consumed outpoints so declared and
    /// auxiliary inputs cannot accidentally reuse the same UTXO.
    pub(crate) fn new(resolver: &'a Resolver<'a, WalletProvider>) -> Self {
        Self {
            used_outpoints: HashSet::new(),
            resolver,
            _wallet_provider: PhantomData,
        }
    }

    /// Resolve one declared input from either provided or wallet source.
    pub(crate) async fn resolve_declared_input_material(
        &mut self,
        input: &InputSchema,
        supply_and_demand: &SupplyAndDemand,
    ) -> Result<ResolvedInputMaterial, WalletAbiError> {
        match &input.utxo_source {
            UTXOSource::Wallet { filter } => {
                self.resolve_wallet_input_material(input, filter, supply_and_demand)
                    .await
            }
            UTXOSource::Provided { outpoint } => {
                self.resolve_provided_input_material(input, *outpoint).await
            }
        }
    }

    /// Reserve an outpoint as soon as it is chosen so duplicate spending is
    /// rejected at resolution time instead of later in PSET construction.
    pub(crate) fn reserve_outpoint(
        &mut self,
        item_id: &str,
        outpoint: OutPoint,
    ) -> Result<(), WalletAbiError> {
        if self.used_outpoints.insert(outpoint) {
            return Ok(());
        }

        Err(WalletAbiError::InvalidRequest(format!(
            "duplicate outpoint resolved for '{}': {}:{}",
            item_id, outpoint.txid, outpoint.vout
        )))
    }

    /// Expose already-consumed outpoints so auxiliary funding selection can
    /// skip inputs the resolver has already committed to use.
    pub(crate) fn used_outpoints(&self) -> &HashSet<OutPoint> {
        &self.used_outpoints
    }

    /// Resolve input material from wallet snapshot using deficit-aware selection.
    async fn resolve_wallet_input_material(
        &mut self,
        input: &InputSchema,
        filter: &WalletSourceFilter,
        supply_and_demand: &SupplyAndDemand,
    ) -> Result<ResolvedInputMaterial, WalletAbiError> {
        if !matches!(input.unblinding, InputUnblinding::Wallet) {
            return Err(WalletAbiError::InvalidRequest(format!(
                "input '{}' uses utxo_source=wallet but unblinding is not 'wallet'",
                input.id
            )));
        }

        let selected_index = self
            .filter_tx_out_index(filter, supply_and_demand)?
            .ok_or_else(|| {
                WalletAbiError::Funding(format!(
                    "no wallet UTXO matched contract input '{}' filter",
                    input.id
                ))
            })?;

        let selected = self
            .resolver
            .wallet_snapshot()
            .get(selected_index)
            .ok_or_else(|| {
                WalletAbiError::InvalidResponse(format!(
                    "wallet snapshot index {selected_index} missing while resolving input '{}'",
                    input.id
                ))
            })?;
        let outpoint = selected.outpoint;
        let tx_out = selected.txout.clone();
        let secrets = selected.unblinded;
        let wallet_finalization_weight = Some(selected.max_weight_to_satisfy);

        self.reserve_outpoint(&input.id, outpoint)?;

        Ok(ResolvedInputMaterial {
            outpoint,
            tx_out,
            secrets,
            wallet_finalization_weight,
        })
    }

    /// Resolve input material from a provided outpoint and optional unblinding hints.
    async fn resolve_provided_input_material(
        &mut self,
        input: &InputSchema,
        outpoint: OutPoint,
    ) -> Result<ResolvedInputMaterial, WalletAbiError> {
        self.reserve_outpoint(&input.id, outpoint)?;

        let tx_out = self.resolver.wallet_provider().get_tx_out(outpoint).await?;

        let secrets = match &input.unblinding {
            InputUnblinding::Wallet => self.resolver.wallet_provider().unblind(&tx_out)?,
            InputUnblinding::Provided { secret_key } => tx_out
                .unblind(&lwk_wollet::EC, *secret_key)
                .map_err(|error| {
                    WalletAbiError::InvalidRequest(format!(
                        "unable to unblind input '{}' with provided unblinding key: {error}",
                        input.id
                    ))
                })?,
            InputUnblinding::Explicit => {
                let (Asset::Explicit(asset), Value::Explicit(value)) = (tx_out.asset, tx_out.value)
                else {
                    return Err(WalletAbiError::InvalidRequest(format!(
                        "input '{}' is marked explicit but the provided prevout is confidential",
                        input.id
                    )));
                };

                TxOutSecrets {
                    asset,
                    asset_bf: AssetBlindingFactor::zero(),
                    value,
                    value_bf: ValueBlindingFactor::zero(),
                }
            }
        };

        Ok(ResolvedInputMaterial {
            outpoint,
            tx_out,
            secrets,
            wallet_finalization_weight: None,
        })
    }

    /// Choose the best wallet UTXO index that satisfies the caller filter while
    /// minimizing the remaining supply deficit tracked by the balancer.
    fn filter_tx_out_index(
        &self,
        filter: &WalletSourceFilter,
        supply_and_demand: &SupplyAndDemand,
    ) -> Result<Option<usize>, WalletAbiError> {
        let mut best: Option<(usize, CandidateScore)> = None;
        let current_total_deficit = supply_and_demand.total_remaining_deficit()?;

        for (index, candidate) in self.resolver.wallet_snapshot().iter().enumerate() {
            if !self.matches_wallet_filter(candidate, filter) {
                continue;
            }

            let score = supply_and_demand.score_candidate(candidate, current_total_deficit)?;

            match &best {
                Some((_, best_score)) if score >= *best_score => {}
                _ => best = Some((index, score)),
            }
        }

        Ok(best.map(|(index, _)| index))
    }

    /// Centralize wallet filter matching so every wallet-sourced input is
    /// interpreted with the same asset, amount, and lock semantics.
    fn matches_wallet_filter(&self, candidate: &ExternalUtxo, filter: &WalletSourceFilter) -> bool {
        if self.used_outpoints.contains(&candidate.outpoint) {
            return false;
        }

        let asset_ok = match filter.asset {
            AssetFilter::None => true,
            AssetFilter::Exact { asset_id } => candidate.unblinded.asset == asset_id,
        };
        if !asset_ok {
            return false;
        }

        let amount_ok = match filter.amount {
            AmountFilter::None => true,
            AmountFilter::Exact { amount_sat } => candidate.unblinded.value == amount_sat,
            AmountFilter::Min { amount_sat } => candidate.unblinded.value >= amount_sat,
        };
        if !amount_ok {
            return false;
        }

        match &filter.lock {
            LockFilter::None => true,
            LockFilter::Script { script } => candidate.txout.script_pubkey == *script,
        }
    }
}
