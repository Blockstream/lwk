use crate::error::WalletAbiError;
use crate::wallet_abi::schema::{RuntimeParams, WalletProviderMeta, WalletRequestSession};
use crate::wallet_abi::tx_resolution::input_material::InputMaterialResolver;
use crate::wallet_abi::tx_resolution::resolution_artifact::ResolutionArtifacts;

use std::sync::Arc;

use lwk_wollet::elements::pset::PartiallySignedTransaction;
use lwk_wollet::ExternalUtxo;

pub(super) struct Resolver<'a, WalletProvider: WalletProviderMeta> {
    wallet_request_session: &'a WalletRequestSession,
    wallet_provider: &'a WalletProvider,
    fee_target_sat: u64,
}

impl<'a, WalletProvider: WalletProviderMeta> Resolver<'a, WalletProvider>
where
    WalletAbiError: From<WalletProvider::Error>,
{
    pub(crate) fn new(
        wallet_request_session: &'a WalletRequestSession,
        wallet_provider: &'a WalletProvider,
        fee_target_sat: u64,
    ) -> Self {
        Self {
            wallet_request_session,
            wallet_provider,
            fee_target_sat,
        }
    }

    pub(crate) fn wallet_provider(&self) -> &WalletProvider {
        self.wallet_provider
    }

    pub(crate) fn wallet_snapshot(&self) -> &Arc<[ExternalUtxo]> {
        &self.wallet_request_session.spendable_utxos
    }

    pub(crate) async fn resolve_request(
        &self,
        runtime_params: &RuntimeParams,
        mut pst: PartiallySignedTransaction,
    ) -> Result<PartiallySignedTransaction, WalletAbiError> {
        let mut input_material_resolver = InputMaterialResolver::new(self);

        for (input_index, input) in runtime_params.inputs.iter().enumerate() {
            let material = input_material_resolver
                .resolve_declared_input_material(input)
                .await?;

            todo!()
        }

        todo!()
    }

    pub(crate) fn get_resolution_artifact(&self) -> ResolutionArtifacts {
        todo!()
    }
}
