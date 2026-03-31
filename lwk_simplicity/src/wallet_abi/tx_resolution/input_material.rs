use crate::error::WalletAbiError;
use crate::wallet_abi::schema::{
    InputSchema, InputUnblinding, UTXOSource, WalletProviderMeta, WalletSourceFilter,
};
use crate::wallet_abi::tx_resolution::resolver::Resolver;

use std::collections::HashSet;
use std::marker::PhantomData;

use lwk_wollet::elements::confidential::{Asset, AssetBlindingFactor, Value, ValueBlindingFactor};
use lwk_wollet::elements::{OutPoint, TxOut};

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

impl<'a, WalletProvider: WalletProviderMeta> InputMaterialResolver<'a, WalletProvider>
where
    WalletAbiError: From<WalletProvider::Error>,
{
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
    ) -> Result<ResolvedInputMaterial, WalletAbiError> {
        match &input.utxo_source {
            UTXOSource::Wallet { filter } => {
                self.resolve_wallet_input_material(input, filter).await
            }
            UTXOSource::Provided { outpoint } => {
                self.resolve_provided_input_material(input, *outpoint).await
            }
        }
    }

    /// Resolve input material from wallet snapshot using deficit-aware selection.
    async fn resolve_wallet_input_material(
        &mut self,
        input: &InputSchema,
        filter: &WalletSourceFilter,
    ) -> Result<ResolvedInputMaterial, WalletAbiError> {
        if !matches!(input.unblinding, InputUnblinding::Wallet) {
            return Err(WalletAbiError::InvalidRequest(format!(
                "input '{}' uses utxo_source=wallet but unblinding is not 'wallet'",
                input.id
            )));
        }

        let selected_index = self.filter_tx_out_index(filter)?.ok_or_else(|| {
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

    fn filter_tx_out_index(
        &self,
        _filter: &WalletSourceFilter,
    ) -> Result<Option<usize>, WalletAbiError> {
        todo!()
    }

    fn reserve_outpoint(
        &mut self,
        input_id: &str,
        outpoint: OutPoint,
    ) -> Result<(), WalletAbiError> {
        if self.used_outpoints.insert(outpoint) {
            return Ok(());
        }

        Err(WalletAbiError::InvalidRequest(format!(
            "duplicate input outpoint resolved for '{}': {}:{}",
            input_id, outpoint.txid, outpoint.vout
        )))
    }
}
