use crate::wallet_abi::schema::{RuntimeParams, WalletProviderMeta, WalletRequestSession};
use crate::wallet_abi::tx_resolution::resolution_artifact::ResolutionArtifacts;

use lwk_wollet::elements::pset::PartiallySignedTransaction;

pub(super) struct ResolutionState<'a, WalletProvider: WalletProviderMeta> {
    wallet_request_session: &'a WalletRequestSession,
    wallet_provider: &'a WalletProvider,
    fee_target_sat: u64,
}

impl<'a, WalletProvider: WalletProviderMeta> ResolutionState<'a, WalletProvider> {
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

    pub(crate) fn resolve_request(
        &self,
        runtime_params: &RuntimeParams,
    ) -> PartiallySignedTransaction {
        todo!()
    }

    pub(crate) fn get_resolution_artifact(&self) -> ResolutionArtifacts {
        todo!()
    }
}
