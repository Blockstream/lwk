//! Runtime transaction builder/finalizer.

use crate::error::WalletAbiError;
use crate::wallet_abi::schema::{
    KeyStoreMeta, TransactionInfo, TxCreateRequest, TxCreateResponse, WalletProviderMeta,
    WalletRequestSession, WalletRuntimeDeps, WalletSessionFactory,
};
use crate::wallet_abi::tx_resolution::input_finalizer::{
    extract_env_utxos, finalize_simf_inputs, finalize_wallet_inputs,
};
use crate::wallet_abi::tx_resolution::resolution_artifact::ResolutionArtifacts;
use crate::wallet_abi::tx_resolution::resolver::Resolver;

use log::{error, warn};

use lwk_common::{calculate_fee, DEFAULT_FEE_RATE};

use lwk_wollet::elements::hex::ToHex;
use lwk_wollet::elements::pset::serialize::Serialize;
use lwk_wollet::elements::pset::PartiallySignedTransaction;
use lwk_wollet::elements::Transaction;

pub struct Runtime<'a, Signer, SessionFactory, WalletProvider>
where
    Signer: KeyStoreMeta,
    SessionFactory: WalletSessionFactory,
    WalletProvider: WalletProviderMeta,
{
    request: TxCreateRequest,
    signer_meta: &'a Signer,
    wallet_deps: &'a WalletRuntimeDeps<SessionFactory, WalletProvider>,
}

impl<'a, Signer, SessionFactory, WalletProvider> Runtime<'a, Signer, SessionFactory, WalletProvider>
where
    Signer: KeyStoreMeta,
    SessionFactory: WalletSessionFactory,
    WalletProvider: WalletProviderMeta,
    WalletAbiError: From<Signer::Error> + From<SessionFactory::Error> + From<WalletProvider::Error>,
{
    /// Capture the request and runtime dependencies so the build can reuse one
    /// signer/session context without cloning mutable state.
    pub fn new(
        request: TxCreateRequest,
        signer_meta: &'a Signer,
        wallet_deps: &'a WalletRuntimeDeps<SessionFactory, WalletProvider>,
    ) -> Self {
        Self {
            request,
            signer_meta,
            wallet_deps,
        }
    }

    /// Drive the full runtime flow so request validation, building,
    /// finalization, and optional broadcast all happen under one consistent
    /// wallet session and fee policy.
    pub async fn process_request(&self) -> Result<TxCreateResponse, WalletAbiError> {
        let wallet_session = self.open_session().await?;
        let fee_rate_sat_kvb = self.get_fee_rate()?;

        let resolved_tx = self
            .resolve_transaction(&wallet_session, fee_rate_sat_kvb)
            .await?;

        self.process_response(resolved_tx).await
    }

    /// Build and finalize the transaction, then verify the realized fee and
    /// local amount proofs before handing the transaction back to the API layer.
    async fn resolve_transaction(
        &self,
        wallet_session: &WalletRequestSession,
        fee_rate_sat_kvb: f32,
    ) -> Result<Transaction, WalletAbiError> {
        let estimated_fee = self
            .estimate_fee_target(fee_rate_sat_kvb, wallet_session)
            .await?;

        let (pst, artifacts) = self
            .build_transaction(estimated_fee, wallet_session)
            .await?;

        let pst = finalize_simf_inputs(
            self.signer_meta,
            pst,
            artifacts.finalizers(),
            wallet_session.network.into(),
        )?;
        let pst = finalize_wallet_inputs(self.signer_meta, pst, artifacts.finalizers())?;

        let final_fee = calculate_fee(pst.extract_tx()?.discount_weight(), fee_rate_sat_kvb);
        if estimated_fee < final_fee {
            error!(
                "fee estimation under-shot; target={estimated_fee} sat, realized={final_fee} sat"
            );

            return Err(WalletAbiError::Funding("fee estimation failed".to_string()));
        }

        if estimated_fee != final_fee {
            warn!(
                "fee estimate exceeded realized fee; target={estimated_fee} sat, realized={final_fee} sat"
            );
        }

        let utxos = extract_env_utxos(&pst)?;

        let tx = pst.extract_tx()?;

        // `elements::Transaction::verify_tx_amt_proofs` treats zero-value OP_RETURN outputs
        // as a hard error even though Elements accepts them as provably unspendable. Lending
        // contracts use these outputs for metadata and burns, so skip the local proof check
        // for that specific transaction shape and rely on node validation instead.
        if !tx.output.iter().any(|tx_out| {
            tx_out.script_pubkey.is_provably_unspendable() && tx_out.value.explicit() == Some(0)
        }) {
            tx.verify_tx_amt_proofs(&lwk_wollet::EC, &utxos)?;
        }

        Ok(tx)
    }

    /// Estimate the fee target from a provisional build so the final
    /// transaction can be constructed against measured weight instead of a
    /// static guess.
    async fn estimate_fee_target(
        &self,
        fee_rate_sat_kvb: f32,
        wallet_session: &WalletRequestSession,
    ) -> Result<u64, WalletAbiError> {
        // TODO: figure out the better way to build estimation transaction
        let (fee_estimation_build, artifacts) = self.build_transaction(1, wallet_session).await?;

        let fee_estimation_build = finalize_simf_inputs(
            self.signer_meta,
            fee_estimation_build,
            artifacts.finalizers(),
            wallet_session.network.into(),
        )?;

        Ok(calculate_fee(
            fee_estimation_build.extract_tx()?.discount_weight()
                + artifacts.wallet_input_finalization_weight(),
            fee_rate_sat_kvb,
        ))
    }

    /// Build and blind a provisional transaction for one fee target so later
    /// finalization passes operate on the exact output set the runtime intends
    /// to publish.
    async fn build_transaction(
        &self,
        fee_target_sat: u64,
        wallet_session: &WalletRequestSession,
    ) -> Result<(PartiallySignedTransaction, ResolutionArtifacts), WalletAbiError> {
        let mut pst = PartiallySignedTransaction::new_v2();
        pst.global.tx_data.fallback_locktime = self.request.params.lock_time;

        let resolver = Resolver::new(
            wallet_session,
            &self.wallet_deps.wallet_provider,
            fee_target_sat,
        );

        let (mut pst, artifacts) = resolver.resolve_request(&self.request.params, pst).await?;

        pst.blind_last(
            &mut lwk_wollet::secp256k1::rand::thread_rng(),
            &lwk_wollet::EC,
            artifacts.secrets(),
        )?;

        Ok((pst, artifacts))
    }

    /// Convert the resolved transaction into API response form and, when
    /// broadcasting, verify the provider echoed the same txid to catch backend
    /// mismatches before reporting success.
    async fn process_response(
        &self,
        resolved_tx: Transaction,
    ) -> Result<TxCreateResponse, WalletAbiError> {
        let txid = resolved_tx.txid();

        if self.request.broadcast {
            let published_txid = self
                .wallet_deps
                .wallet_provider
                .broadcast_transaction(&resolved_tx)
                .await?;
            if txid != published_txid {
                error!("broadcast txid mismatch: locally built txid={txid}, esplora returned txid={published_txid}");

                return Err(WalletAbiError::InvalidResponse(
                    "broadcast txid mismatch".to_string(),
                ));
            }
        }

        Ok(TxCreateResponse::ok(
            &self.request,
            TransactionInfo {
                tx_hex: resolved_tx.serialize().to_hex(),
                txid,
            },
            None,
        ))
    }

    /// Open and validate the wallet session up front so all later resolution
    /// logic runs against a network-compatible snapshot.
    async fn open_session(&self) -> Result<WalletRequestSession, WalletAbiError> {
        let wallet_session = self
            .wallet_deps
            .session_factory
            .open_wallet_request_session()
            .await?;

        self.request.validate_for_runtime(wallet_session.network)?;

        Ok(wallet_session)
    }

    /// Normalize the request fee rate once so every build phase shares the same
    /// finite, non-negative sat/kvB target.
    fn get_fee_rate(&self) -> Result<f32, WalletAbiError> {
        let fee_rate_sat_kvb = self
            .request
            .params
            .fee_rate_sat_kvb
            .unwrap_or(DEFAULT_FEE_RATE);

        if !fee_rate_sat_kvb.is_finite() {
            return Err(WalletAbiError::InvalidRequest(format!(
                "invalid fee rate (sat/kvB): expected finite value, got {fee_rate_sat_kvb}"
            )));
        }
        if fee_rate_sat_kvb < 0.0 {
            return Err(WalletAbiError::InvalidRequest(format!(
                "invalid fee rate (sat/kvB): expected non-negative value, got {fee_rate_sat_kvb}"
            )));
        }

        Ok(fee_rate_sat_kvb)
    }
}
