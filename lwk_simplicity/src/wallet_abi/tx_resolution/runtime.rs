//! Runtime transaction builder/finalizer.

use crate::error::WalletAbiError;
use crate::wallet_abi::schema::{
    KeyStoreMeta, TransactionInfo, TxCreateRequest, TxCreateResponse, WalletProviderMeta,
    WalletRequestSession, WalletRuntimeDeps, WalletSessionFactory,
};

use log::error;

use lwk_common::DEFAULT_FEE_RATE;

use lwk_wollet::elements::hex::ToHex;
use lwk_wollet::elements::pset::serialize::Serialize;
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

    pub async fn process_request(&self) -> Result<TxCreateResponse, WalletAbiError> {
        let wallet_session = self.open_session().await?;
        let fee_rate_sat_kvb = self.get_fee_rate()?;

        let resolved_tx = self
            .resolve_transaction(&wallet_session, fee_rate_sat_kvb)
            .await?;

        self.process_response(resolved_tx).await
    }

    async fn resolve_transaction(
        &self,
        _wallet_session: &WalletRequestSession,
        _fee_rate_sat_kvb: f32,
    ) -> Result<Transaction, WalletAbiError> {
        todo!()
    }

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

    async fn open_session(&self) -> Result<WalletRequestSession, WalletAbiError> {
        let wallet_session = self
            .wallet_deps
            .session_factory
            .open_wallet_request_session()
            .await?;

        self.request.validate_for_runtime(wallet_session.network)?;

        Ok(wallet_session)
    }

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
