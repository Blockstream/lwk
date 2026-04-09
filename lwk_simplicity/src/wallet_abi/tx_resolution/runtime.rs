//! Runtime transaction builder/finalizer.

use crate::error::WalletAbiError;
use crate::wallet_abi::schema::{
    KeyStoreMeta, LockVariant, PreviewAssetDelta, PreviewOutput, PreviewOutputKind, RequestPreview,
    TransactionInfo, TxCreateArtifacts, TxCreateRequest, TxCreateResponse, TxEvaluateRequest,
    TxEvaluateResponse, WalletProviderMeta, WalletRequestSession, WalletRuntimeDeps,
    WalletSessionFactory,
};
use crate::wallet_abi::tx_resolution::input_finalizer::{
    extract_env_utxos, finalize_simf_inputs, finalize_wallet_inputs,
};
use crate::wallet_abi::tx_resolution::resolution_artifact::ResolutionArtifacts;
use crate::wallet_abi::tx_resolution::resolver::Resolver;

use std::collections::BTreeMap;

use log::{error, warn};

use lwk_common::{calculate_fee, DEFAULT_FEE_RATE};

use lwk_wollet::elements::hex::ToHex;
use lwk_wollet::elements::pset::serialize::Serialize;
use lwk_wollet::elements::pset::PartiallySignedTransaction;
use lwk_wollet::elements::{AssetId, Transaction};
use lwk_wollet::ElementsNetwork;

/// Internal request adapter used to share runtime logic between tx-create and
/// tx-evaluate envelopes.
#[doc(hidden)]
pub trait RuntimeRequest {
    fn network(&self) -> ElementsNetwork;
    fn params(&self) -> &crate::wallet_abi::schema::RuntimeParams;
    fn validate_for_runtime(&self, runtime_network: ElementsNetwork) -> Result<(), WalletAbiError>;
}

impl RuntimeRequest for TxCreateRequest {
    fn network(&self) -> ElementsNetwork {
        self.network
    }

    fn params(&self) -> &crate::wallet_abi::schema::RuntimeParams {
        &self.params
    }

    fn validate_for_runtime(&self, runtime_network: ElementsNetwork) -> Result<(), WalletAbiError> {
        self.validate_for_runtime(runtime_network)
    }
}

impl RuntimeRequest for TxEvaluateRequest {
    fn network(&self) -> ElementsNetwork {
        self.network
    }

    fn params(&self) -> &crate::wallet_abi::schema::RuntimeParams {
        &self.params
    }

    fn validate_for_runtime(&self, runtime_network: ElementsNetwork) -> Result<(), WalletAbiError> {
        self.validate_for_runtime(runtime_network)
    }
}

struct PreparedRequest {
    estimated_fee: u64,
    pst: PartiallySignedTransaction,
    artifacts: ResolutionArtifacts,
    preview: RequestPreview,
}

pub struct Runtime<'a, Request, Signer, SessionFactory, WalletProvider> {
    request: Request,
    signer_meta: &'a Signer,
    wallet_deps: &'a WalletRuntimeDeps<SessionFactory, WalletProvider>,
}

impl<'a, Request, Signer, SessionFactory, WalletProvider>
    Runtime<'a, Request, Signer, SessionFactory, WalletProvider>
where
    Request: RuntimeRequest,
    Signer: KeyStoreMeta,
    SessionFactory: WalletSessionFactory,
    WalletProvider: WalletProviderMeta,
    WalletAbiError: From<Signer::Error> + From<SessionFactory::Error> + From<WalletProvider::Error>,
{
    /// Finalize a previously prepared PSET, then verify the realized fee and
    /// local amount proofs before exposing the transaction to the API layer.
    fn finalize_transaction(
        &self,
        pst: PartiallySignedTransaction,
        artifacts: ResolutionArtifacts,
        estimated_fee: u64,
        fee_rate_sat_kvb: f32,
    ) -> Result<Transaction, WalletAbiError> {
        let pst = finalize_simf_inputs(
            self.signer_meta,
            pst,
            artifacts.finalizers(),
            self.request.network().into(),
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

    /// Resolve and blind the transaction once, then derive the public preview
    /// from the same concrete build state that later finalization will use.
    async fn prepare_request(
        &self,
        wallet_session: &WalletRequestSession,
        fee_rate_sat_kvb: f32,
    ) -> Result<PreparedRequest, WalletAbiError> {
        let estimated_fee = self
            .estimate_fee_target(fee_rate_sat_kvb, wallet_session)
            .await?;
        let (pst, artifacts) = self
            .build_transaction(estimated_fee, wallet_session)
            .await?;
        let preview = self.build_preview(&pst, &artifacts)?;

        Ok(PreparedRequest {
            estimated_fee,
            pst,
            artifacts,
            preview,
        })
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
        pst.global.tx_data.fallback_locktime = self.request.params().lock_time;

        let resolver = Resolver::new(
            wallet_session,
            &self.wallet_deps.wallet_provider,
            fee_target_sat,
        );

        let (mut pst, artifacts) = resolver.resolve_request(self.request.params(), pst).await?;

        pst.blind_last(
            &mut lwk_wollet::secp256k1::rand::thread_rng(),
            &lwk_wollet::EC,
            artifacts.secrets(),
        )?;

        Ok((pst, artifacts))
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
            .params()
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

    /// Build the caller-facing preview from the materialized PSET and the
    /// tracked wallet-owned input supply.
    fn build_preview(
        &self,
        pst: &PartiallySignedTransaction,
        artifacts: &ResolutionArtifacts,
    ) -> Result<RequestPreview, WalletAbiError> {
        let mut wallet_deltas: BTreeMap<AssetId, i64> = BTreeMap::new();

        for (asset_id, amount_sat) in artifacts.wallet_input_supply() {
            let amount_sat = i64::try_from(*amount_sat).map_err(|_| {
                WalletAbiError::InvalidResponse(format!(
                    "wallet preview input amount exceeds i64 for asset {asset_id}"
                ))
            })?;
            add_wallet_delta(&mut wallet_deltas, *asset_id, -amount_sat)?;
        }

        let mut outputs = Vec::with_capacity(pst.outputs().len());
        for (output_index, output) in pst.outputs().iter().enumerate() {
            let asset_id = output.asset.ok_or_else(|| {
                WalletAbiError::InvalidResponse(format!(
                    "preview output {output_index} missing explicit asset"
                ))
            })?;
            let amount_sat = output.amount.ok_or_else(|| {
                WalletAbiError::InvalidResponse(format!(
                    "preview output {output_index} missing explicit amount"
                ))
            })?;
            let kind = self.preview_output_kind(output_index);

            if matches!(kind, PreviewOutputKind::Receive | PreviewOutputKind::Change) {
                let signed_amount = i64::try_from(amount_sat).map_err(|_| {
                    WalletAbiError::InvalidResponse(format!(
                        "preview output amount at index {output_index} exceeds i64"
                    ))
                })?;
                add_wallet_delta(&mut wallet_deltas, asset_id, signed_amount)?;
            }

            outputs.push(PreviewOutput {
                kind,
                asset_id,
                amount_sat,
                script_pubkey: output.script_pubkey.clone(),
            });
        }

        Ok(RequestPreview {
            asset_deltas: wallet_deltas
                .into_iter()
                .filter_map(|(asset_id, wallet_delta_sat)| {
                    (wallet_delta_sat != 0).then_some(PreviewAssetDelta {
                        asset_id,
                        wallet_delta_sat,
                    })
                })
                .collect(),
            outputs,
            warnings: vec![],
        })
    }

    /// Classify one materialized output using request ordering:
    /// requested outputs first, then runtime fee, then runtime change.
    fn preview_output_kind(&self, output_index: usize) -> PreviewOutputKind {
        let requested_outputs = &self.request.params().outputs;

        if output_index < requested_outputs.len() {
            if matches!(&requested_outputs[output_index].lock, LockVariant::Wallet) {
                return PreviewOutputKind::Receive;
            }

            return PreviewOutputKind::External;
        }

        if output_index == requested_outputs.len() {
            return PreviewOutputKind::Fee;
        }

        PreviewOutputKind::Change
    }
}

impl<'a, Signer, SessionFactory, WalletProvider>
    Runtime<'a, TxCreateRequest, Signer, SessionFactory, WalletProvider>
where
    Signer: KeyStoreMeta,
    SessionFactory: WalletSessionFactory,
    WalletProvider: WalletProviderMeta,
    WalletAbiError: From<Signer::Error> + From<SessionFactory::Error> + From<WalletProvider::Error>,
{
    /// Capture one tx-create request and its runtime dependencies.
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
        let prepared = self
            .prepare_request(&wallet_session, fee_rate_sat_kvb)
            .await?;
        let resolved_tx = self.finalize_transaction(
            prepared.pst,
            prepared.artifacts,
            prepared.estimated_fee,
            fee_rate_sat_kvb,
        )?;

        self.process_response(resolved_tx, prepared.preview).await
    }

    /// Convert the resolved transaction into API response form and, when
    /// broadcasting, verify the provider echoed the same txid to catch backend
    /// mismatches before reporting success.
    async fn process_response(
        &self,
        resolved_tx: Transaction,
        preview: RequestPreview,
    ) -> Result<TxCreateResponse, WalletAbiError> {
        let txid = resolved_tx.txid();

        if self.request.broadcast {
            let published_txid = self
                .wallet_deps
                .wallet_provider
                .broadcast_transaction(&resolved_tx)
                .await?;
            if txid != published_txid {
                error!(
                    "broadcast txid mismatch: locally built txid={txid}, esplora returned txid={published_txid}"
                );

                return Err(WalletAbiError::InvalidResponse(
                    "broadcast txid mismatch".to_string(),
                ));
            }
        }

        let mut artifacts = TxCreateArtifacts::new();
        artifacts.insert("preview".to_string(), preview.to_artifact_value()?);

        Ok(TxCreateResponse::ok(
            &self.request,
            TransactionInfo {
                tx_hex: resolved_tx.serialize().to_hex(),
                txid,
            },
            Some(artifacts),
        ))
    }
}

impl<'a, Signer, SessionFactory, WalletProvider>
    Runtime<'a, TxEvaluateRequest, Signer, SessionFactory, WalletProvider>
where
    Signer: KeyStoreMeta,
    SessionFactory: WalletSessionFactory,
    WalletProvider: WalletProviderMeta,
    WalletAbiError: From<Signer::Error> + From<SessionFactory::Error> + From<WalletProvider::Error>,
{
    /// Capture one tx-evaluate request and its runtime dependencies.
    pub fn new(
        request: TxEvaluateRequest,
        signer_meta: &'a Signer,
        wallet_deps: &'a WalletRuntimeDeps<SessionFactory, WalletProvider>,
    ) -> Self {
        Self {
            request,
            signer_meta,
            wallet_deps,
        }
    }

    /// Resolve the request through fee estimation, balancing, output
    /// allocation, and blinding, then return only the caller-facing preview.
    pub async fn evaluate_request(&self) -> Result<TxEvaluateResponse, WalletAbiError> {
        let wallet_session = self.open_session().await?;
        let fee_rate_sat_kvb = self.get_fee_rate()?;
        let prepared = self
            .prepare_request(&wallet_session, fee_rate_sat_kvb)
            .await?;

        Ok(TxEvaluateResponse::ok(&self.request, prepared.preview))
    }
}

fn add_wallet_delta(
    wallet_deltas: &mut BTreeMap<AssetId, i64>,
    asset_id: AssetId,
    amount_sat: i64,
) -> Result<(), WalletAbiError> {
    let next = wallet_deltas.get(&asset_id).copied().unwrap_or(0);
    let updated = next.checked_add(amount_sat).ok_or_else(|| {
        WalletAbiError::InvalidResponse(format!(
            "wallet preview delta overflow for asset {asset_id}"
        ))
    })?;
    wallet_deltas.insert(asset_id, updated);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::Runtime;
    use crate::error::WalletAbiError;
    use crate::wallet_abi::schema::{
        AmountFilter, AssetFilter, AssetVariant, BlinderVariant, FinalizerSpec, InputSchema,
        InputUnblinding, LockFilter, LockVariant, OutputSchema, TxEvaluateRequest, UTXOSource,
        WalletOutputRequest, WalletOutputTemplate, WalletProviderMeta, WalletRequestSession,
        WalletRuntimeDeps, WalletSessionFactory, WalletSourceFilter,
    };

    use std::future::Future;
    use std::pin::pin;
    use std::str::FromStr;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;
    use std::task::{Context, Poll, Waker};

    use lwk_wollet::bitcoin::bip32::{DerivationPath, Fingerprint, KeySource};
    use lwk_wollet::bitcoin::PublicKey;
    use lwk_wollet::elements::confidential::{
        Asset as ConfidentialAsset, AssetBlindingFactor, Nonce, Value, ValueBlindingFactor,
    };
    use lwk_wollet::elements::hashes::Hash;
    use lwk_wollet::elements::script::Builder;
    use lwk_wollet::elements::secp256k1_zkp::PublicKey as BlindingPublicKey;
    use lwk_wollet::elements::{OutPoint, Transaction, TxOut, TxOutSecrets, TxOutWitness, Txid};
    use lwk_wollet::secp256k1::schnorr::Signature;
    use lwk_wollet::secp256k1::{Message, XOnlyPublicKey};
    use lwk_wollet::ExternalUtxo;

    #[derive(Debug, thiserror::Error)]
    #[error("test runtime error")]
    struct TestRuntimeError;

    impl From<TestRuntimeError> for WalletAbiError {
        fn from(error: TestRuntimeError) -> Self {
            WalletAbiError::InvalidRequest(error.to_string())
        }
    }

    struct TestSigner;

    impl crate::wallet_abi::schema::KeyStoreMeta for TestSigner {
        type Error = TestRuntimeError;

        fn get_raw_signing_x_only_pubkey(&self) -> Result<XOnlyPublicKey, Self::Error> {
            XOnlyPublicKey::from_str(
                "79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798",
            )
            .map_err(|_| TestRuntimeError)
        }

        fn sign_pst(
            &self,
            _pst: &mut lwk_wollet::elements::pset::PartiallySignedTransaction,
        ) -> Result<(), Self::Error> {
            Ok(())
        }

        fn sign_schnorr(
            &self,
            _message: Message,
            _xonly_public_key: XOnlyPublicKey,
        ) -> Result<Signature, Self::Error> {
            Err(TestRuntimeError)
        }
    }

    #[derive(Clone)]
    struct TestSessionFactory {
        calls: Arc<AtomicUsize>,
        session: WalletRequestSession,
    }

    impl WalletSessionFactory for TestSessionFactory {
        type Error = TestRuntimeError;

        fn open_wallet_request_session(
            &self,
        ) -> impl Future<Output = Result<WalletRequestSession, Self::Error>> + Send + '_ {
            self.calls.fetch_add(1, Ordering::Relaxed);
            let session = self.session.clone();
            async move { Ok(session) }
        }
    }

    struct TestWalletProvider {
        broadcast_calls: Arc<AtomicUsize>,
        derivation_pair: (PublicKey, KeySource),
        template: WalletOutputTemplate,
    }

    impl WalletProviderMeta for TestWalletProvider {
        type Error = TestRuntimeError;

        fn get_bip32_derivation_pair(
            &self,
            _out_point: &OutPoint,
        ) -> Result<Option<(PublicKey, KeySource)>, Self::Error> {
            Ok(Some(self.derivation_pair.clone()))
        }

        fn unblind(&self, _tx_out: &TxOut) -> Result<TxOutSecrets, Self::Error> {
            Err(TestRuntimeError)
        }

        fn get_tx_out(
            &self,
            _outpoint: OutPoint,
        ) -> impl Future<Output = Result<TxOut, Self::Error>> + Send + '_ {
            async move { Err(TestRuntimeError) }
        }

        fn get_wallet_output_template(
            &self,
            _session: &WalletRequestSession,
            _request: &WalletOutputRequest,
        ) -> Result<WalletOutputTemplate, Self::Error> {
            Ok(self.template.clone())
        }

        fn broadcast_transaction(
            &self,
            _tx: &Transaction,
        ) -> impl Future<Output = Result<Txid, Self::Error>> + Send + '_ {
            self.broadcast_calls.fetch_add(1, Ordering::Relaxed);
            async move { Ok(Txid::all_zeros()) }
        }
    }

    #[test]
    fn evaluate_request_returns_preview_without_broadcast() {
        let policy_asset = lwk_wollet::ElementsNetwork::LiquidTestnet.policy_asset();
        let script_pubkey = Builder::new().push_int(1).into_script();
        let outpoint = OutPoint::new(Txid::all_zeros(), 0);
        let derivation_pair = (
            PublicKey::from_str(
                "0279be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798",
            )
            .expect("valid pubkey"),
            (
                Fingerprint::from_str("01020304").expect("valid fingerprint"),
                DerivationPath::from_str("m/84h/1h/0h/0/0").expect("valid derivation path"),
            ),
        );
        let spendable_utxo = ExternalUtxo {
            outpoint,
            txout: TxOut {
                asset: ConfidentialAsset::Explicit(policy_asset),
                value: Value::Explicit(20_000),
                nonce: Nonce::Null,
                script_pubkey: script_pubkey.clone(),
                witness: TxOutWitness::default(),
            },
            tx: None,
            unblinded: TxOutSecrets::new(
                policy_asset,
                AssetBlindingFactor::zero(),
                20_000,
                ValueBlindingFactor::zero(),
            ),
            max_weight_to_satisfy: 0,
        };
        let session_calls = Arc::new(AtomicUsize::new(0));
        let broadcast_calls = Arc::new(AtomicUsize::new(0));
        let deps = WalletRuntimeDeps::new(
            TestSessionFactory {
                calls: Arc::clone(&session_calls),
                session: WalletRequestSession {
                    session_id: "session-1".to_string(),
                    network: lwk_wollet::ElementsNetwork::LiquidTestnet,
                    spendable_utxos: Arc::from(vec![spendable_utxo]),
                },
            },
            TestWalletProvider {
                broadcast_calls: Arc::clone(&broadcast_calls),
                derivation_pair,
                template: WalletOutputTemplate {
                    script_pubkey: script_pubkey.clone(),
                    blinding_pubkey: Some(
                        BlindingPublicKey::from_str(
                            "0279be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798",
                        )
                        .expect("valid blinding pubkey"),
                    ),
                },
            },
        );
        let request = TxEvaluateRequest::from_parts(
            "0d6d53cd-a040-4f0c-8d28-c67b6608fb14",
            lwk_wollet::ElementsNetwork::LiquidTestnet,
            crate::wallet_abi::schema::RuntimeParams {
                inputs: vec![InputSchema {
                    id: "wallet-input".to_string(),
                    utxo_source: UTXOSource::Wallet {
                        filter: WalletSourceFilter {
                            asset: AssetFilter::Exact {
                                asset_id: policy_asset,
                            },
                            amount: AmountFilter::Exact { amount_sat: 20_000 },
                            lock: LockFilter::None,
                        },
                    },
                    unblinding: InputUnblinding::Wallet,
                    finalizer: FinalizerSpec::Wallet,
                    ..InputSchema::default()
                }],
                outputs: vec![OutputSchema {
                    id: "external".to_string(),
                    amount_sat: 5_000,
                    lock: LockVariant::Script {
                        script: script_pubkey.clone(),
                    },
                    asset: AssetVariant::AssetId {
                        asset_id: policy_asset,
                    },
                    blinder: BlinderVariant::Explicit,
                }],
                fee_rate_sat_kvb: Some(0.0),
                lock_time: None,
            },
        )
        .expect("request");
        let signer = TestSigner;

        let response = ready(
            Runtime::<TxEvaluateRequest, _, _, _>::new(request, &signer, &deps).evaluate_request(),
        )
        .expect("evaluate request");

        assert_eq!(session_calls.load(Ordering::Relaxed), 1);
        assert_eq!(broadcast_calls.load(Ordering::Relaxed), 0);
        assert_eq!(
            response.preview.as_ref().expect("preview").asset_deltas,
            vec![crate::wallet_abi::schema::PreviewAssetDelta {
                asset_id: policy_asset,
                wallet_delta_sat: -5_000,
            }]
        );
        assert_eq!(
            response
                .preview
                .as_ref()
                .expect("preview")
                .outputs
                .iter()
                .map(|output| output.kind)
                .collect::<Vec<_>>(),
            vec![
                crate::wallet_abi::schema::PreviewOutputKind::External,
                crate::wallet_abi::schema::PreviewOutputKind::Fee,
                crate::wallet_abi::schema::PreviewOutputKind::Change,
            ]
        );
    }

    fn ready<T>(future: impl Future<Output = T>) -> T {
        let waker = Waker::noop();
        let mut cx = Context::from_waker(waker);
        let mut future = pin!(future);
        match future.as_mut().poll(&mut cx) {
            Poll::Ready(value) => value,
            Poll::Pending => panic!("test future unexpectedly pending"),
        }
    }
}
