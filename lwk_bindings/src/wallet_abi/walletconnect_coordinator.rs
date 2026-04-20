use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::str::FromStr;
use std::sync::{Arc, Mutex};

use elements::encode::{deserialize, serialize};
use elements::hashes::hex::FromHex;
use elements::hex::ToHex;
use serde::{Deserialize, Serialize};

use crate::network::Network;
use crate::types::{AssetBlindingFactor, AssetId, ValueBlindingFactor};
use crate::{
    ExternalUtxo, LwkError, OutPoint, Transaction, TxOut, TxOutSecrets, WalletAbiErrorInfo,
    WalletAbiProvider, WalletAbiRequestSession, WalletAbiTxCreateRequest,
    WalletAbiTxCreateResponse, WalletAbiTxEvaluateRequest, WalletAbiTxEvaluateResponse,
};

use lwk_simplicity::wallet_abi::schema as abi;
use lwk_simplicity::wallet_abi::{
    GET_RAW_SIGNING_X_ONLY_PUBKEY_METHOD, GET_SIGNER_RECEIVE_ADDRESS_METHOD,
    WALLET_ABI_EVALUATE_REQUEST_METHOD, WALLET_ABI_GET_CAPABILITIES_METHOD,
    WALLET_ABI_PROCESS_REQUEST_METHOD,
};

const SNAPSHOT_SCHEMA_VERSION: u32 = 1;
const WALLET_ABI_WALLETCONNECT_CHAIN_MAINNET: &str = "walabi:liquid";
const WALLET_ABI_WALLETCONNECT_CHAIN_TESTNET: &str = "walabi:testnet-liquid";
const WALLET_ABI_WALLETCONNECT_CHAIN_REGTEST: &str = "walabi:localtest-liquid";

const USER_REJECTED_MESSAGE: &str = "wallet connect request rejected by user";
const SESSION_NOT_FOUND_MESSAGE: &str = "wallet connect session not found";

const SUPPORTED_METHODS: &[&str] = &[
    GET_SIGNER_RECEIVE_ADDRESS_METHOD,
    GET_RAW_SIGNING_X_ONLY_PUBKEY_METHOD,
    WALLET_ABI_GET_CAPABILITIES_METHOD,
    WALLET_ABI_EVALUATE_REQUEST_METHOD,
    WALLET_ABI_PROCESS_REQUEST_METHOD,
];

/// Thin WalletConnect-facing coordinator that owns wallet-side Wallet ABI behavior.
#[derive(uniffi::Object)]
pub struct WalletAbiWalletConnectCoordinator {
    provider: Arc<WalletAbiProvider>,
    wallet_id: String,
    provider_network: Arc<Network>,
    provider_chain_id: String,
    signer_x_only_pubkey: String,
    state: Mutex<CoordinatorState>,
}

/// WalletConnect session proposal surfaced to the coordinator by the app.
#[allow(missing_docs)]
#[derive(uniffi::Record, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WalletAbiWalletConnectSessionProposal {
    pub proposal_id: u64,
    pub pairing_uri: Option<String>,
    pub name: String,
    pub description: Option<String>,
    pub url: Option<String>,
    pub icons: Vec<String>,
    pub required_chain_ids: Vec<String>,
    pub optional_chain_ids: Vec<String>,
    pub required_methods: Vec<String>,
    pub optional_methods: Vec<String>,
    pub required_events: Vec<String>,
    pub optional_events: Vec<String>,
}

/// Active WalletConnect session metadata tracked by the coordinator.
#[allow(missing_docs)]
#[derive(uniffi::Record, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WalletAbiWalletConnectSessionInfo {
    pub topic: String,
    pub chain_id: String,
    pub methods: Vec<String>,
    pub accounts: Vec<String>,
    pub peer_name: Option<String>,
    pub peer_description: Option<String>,
    pub peer_url: Option<String>,
    pub peer_icons: Vec<String>,
}

/// One inbound WalletConnect session request.
#[allow(missing_docs)]
#[derive(uniffi::Record, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WalletAbiWalletConnectSessionRequest {
    pub topic: String,
    pub request_id: u64,
    pub chain_id: String,
    pub method: String,
    pub params_json: String,
}

/// UI overlay kind emitted by the coordinator.
#[allow(missing_docs)]
#[derive(uniffi::Enum, Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WalletAbiWalletConnectOverlayKind {
    ConnectionApproval,
    TransactionApproval,
}

/// Current UI overlay the app should render.
#[allow(missing_docs)]
#[derive(uniffi::Record, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WalletAbiWalletConnectOverlay {
    pub kind: WalletAbiWalletConnectOverlayKind,
    pub chain_id: String,
    pub session_topic: Option<String>,
    pub proposal: Option<WalletAbiWalletConnectSessionProposal>,
    pub request: Option<WalletAbiWalletConnectSessionRequest>,
    pub request_json: Option<String>,
    pub preview_json: Option<String>,
    pub awaiting_transport: bool,
}

/// UI state snapshot for the app.
#[allow(missing_docs)]
#[derive(uniffi::Record, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WalletAbiWalletConnectUiState {
    pub active_sessions: Vec<WalletAbiWalletConnectSessionInfo>,
    pub current_overlay: Option<WalletAbiWalletConnectOverlay>,
    pub queued_overlay_count: u32,
    pub last_error: Option<String>,
    pub pending_action_count: u32,
}

/// Semantic WalletConnect reason kind for session reject/disconnect actions.
#[allow(missing_docs)]
#[derive(uniffi::Enum, Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WalletAbiWalletConnectReasonKind {
    UserRejected,
    UserDisconnected,
    UnsupportedProposal,
    ReplacedSession,
    SessionDeleted,
    InternalError,
}

/// Semantic JSON-RPC error kind for WalletConnect request responses.
#[allow(missing_docs)]
#[derive(uniffi::Enum, Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WalletAbiWalletConnectRpcErrorKind {
    InvalidRequest,
    MethodNotSupported,
    Unauthorized,
    SessionNotFound,
    InternalError,
}

/// Semantic transport action kind returned by the coordinator.
#[allow(missing_docs)]
#[derive(uniffi::Enum, Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WalletAbiWalletConnectTransportActionKind {
    ApproveSession,
    RejectSession,
    RespondSuccess,
    RespondWalletAbiError,
    DisconnectSession,
}

/// Approve-session action payload.
#[allow(missing_docs)]
#[derive(uniffi::Record, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WalletAbiWalletConnectApproveSessionAction {
    pub proposal_id: u64,
    pub chain_id: String,
    pub methods: Vec<String>,
    pub accounts: Vec<String>,
}

/// Reject-session action payload.
#[allow(missing_docs)]
#[derive(uniffi::Record, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WalletAbiWalletConnectRejectSessionAction {
    pub proposal_id: u64,
    pub chain_id: String,
    pub reason_kind: WalletAbiWalletConnectReasonKind,
    pub message: String,
}

/// JSON-RPC success response action payload.
#[allow(missing_docs)]
#[derive(uniffi::Record, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WalletAbiWalletConnectRespondSuccessAction {
    pub topic: String,
    pub chain_id: String,
    pub request_id: u64,
    pub method: String,
    pub result_json: String,
}

/// JSON-RPC error response action payload.
#[allow(missing_docs)]
#[derive(uniffi::Record, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WalletAbiWalletConnectRespondWalletAbiErrorAction {
    pub topic: String,
    pub chain_id: String,
    pub request_id: u64,
    pub method: String,
    pub error_kind: WalletAbiWalletConnectRpcErrorKind,
    pub message: String,
}

/// Disconnect-session action payload.
#[allow(missing_docs)]
#[derive(uniffi::Record, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WalletAbiWalletConnectDisconnectSessionAction {
    pub topic: String,
    pub chain_id: String,
    pub reason_kind: WalletAbiWalletConnectReasonKind,
    pub message: String,
}

/// One semantic transport action the app must execute.
#[allow(missing_docs)]
#[derive(uniffi::Record, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WalletAbiWalletConnectTransportAction {
    pub action_id: String,
    pub kind: WalletAbiWalletConnectTransportActionKind,
    pub approve_session: Option<WalletAbiWalletConnectApproveSessionAction>,
    pub reject_session: Option<WalletAbiWalletConnectRejectSessionAction>,
    pub respond_success: Option<WalletAbiWalletConnectRespondSuccessAction>,
    pub respond_wallet_abi_error: Option<WalletAbiWalletConnectRespondWalletAbiErrorAction>,
    pub disconnect_session: Option<WalletAbiWalletConnectDisconnectSessionAction>,
}

/// Cold-start pending-request reconcile output.
#[allow(missing_docs)]
#[derive(uniffi::Record, Clone, Debug, PartialEq, Eq)]
pub struct WalletAbiWalletConnectPendingRequestsReconcileResult {
    pub actions: Vec<WalletAbiWalletConnectTransportAction>,
    pub requests_to_replay: Vec<WalletAbiWalletConnectSessionRequest>,
}

#[derive(Default)]
struct CoordinatorState {
    active_sessions: BTreeMap<String, WalletAbiWalletConnectSessionInfo>,
    overlays: VecDeque<OverlayState>,
    pending_actions: BTreeMap<String, PendingActionEntry>,
    completed_requests: BTreeMap<RequestKey, CachedResponseOutcome>,
    last_error: Option<String>,
}

#[derive(Clone, Debug)]
enum OverlayState {
    Connection(ConnectionOverlayState),
    Transaction(TransactionOverlayState),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct ConnectionOverlayState {
    proposal: WalletAbiWalletConnectSessionProposal,
    awaiting_transport: bool,
    decision: Option<ConnectionOverlayDecision>,
    pending_action_ids: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
enum ConnectionOverlayDecision {
    Approve,
    Reject,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct TransactionOverlayState {
    request: WalletAbiWalletConnectSessionRequest,
    request_json: String,
    preview_json: String,
    frozen_session: RequestSessionSnapshot,
    awaiting_transport: bool,
    decision: Option<TransactionOverlayDecision>,
    pending_action_ids: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
enum TransactionOverlayDecision {
    Approve { outcome: CachedResponseOutcome },
    Reject { outcome: CachedResponseOutcome },
}

#[derive(Clone, Debug)]
struct PendingActionEntry {
    action: WalletAbiWalletConnectTransportAction,
    state: PendingActionState,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
enum PendingActionState {
    ApproveSession {
        chain_id: String,
        expected_methods: Vec<String>,
        expected_accounts: Vec<String>,
    },
    RejectSession,
    RespondSuccess {
        request_key: RequestKey,
        outcome: CachedResponseOutcome,
    },
    RespondWalletAbiError {
        request_key: RequestKey,
        outcome: CachedResponseOutcome,
    },
    DisconnectSession {
        chain_id: String,
        topic: String,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
struct RequestKey {
    topic: String,
    request_id: u64,
    method: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
enum CachedResponseOutcome {
    Success {
        chain_id: String,
        request_id: u64,
        method: String,
        result_json: String,
    },
    RpcError {
        chain_id: String,
        request_id: u64,
        method: String,
        error_kind: WalletAbiWalletConnectRpcErrorKind,
        message: String,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct CoordinatorSnapshot {
    schema_version: u32,
    wallet_id: String,
    chain_id: String,
    signer_x_only_pubkey: String,
    active_sessions: Vec<WalletAbiWalletConnectSessionInfo>,
    overlays: Vec<OverlaySnapshot>,
    pending_actions: Vec<PendingActionSnapshot>,
    completed_requests: Vec<CompletedRequestSnapshot>,
    last_error: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
enum OverlaySnapshot {
    Connection(ConnectionOverlayState),
    Transaction(TransactionOverlayState),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct PendingActionSnapshot {
    action: WalletAbiWalletConnectTransportAction,
    state: PendingActionState,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct CompletedRequestSnapshot {
    request_key: RequestKey,
    outcome: CachedResponseOutcome,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct RequestSessionSnapshot {
    session_id: String,
    chain_id: String,
    spendable_utxos: Vec<ExternalUtxoSnapshot>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct ExternalUtxoSnapshot {
    outpoint: String,
    txout_hex: String,
    tx_hex: Option<String>,
    asset_id: String,
    asset_bf: String,
    value: u64,
    value_bf: String,
    max_weight_to_satisfy: u32,
}

#[derive(Clone, Debug)]
struct ValidatedProposal {
    chain_id: String,
    methods: Vec<String>,
}

#[uniffi::export]
impl WalletAbiWalletConnectCoordinator {
    /// Build a new coordinator for one wallet/account identity.
    #[uniffi::constructor]
    pub fn new(provider: Arc<WalletAbiProvider>, wallet_id: &str) -> Result<Self, LwkError> {
        let provider_network = provider.get_capabilities()?.network();
        let provider_chain_id = network_to_wallet_connect_chain(provider_network.as_ref())?;
        let signer_x_only_pubkey = provider.get_raw_signing_x_only_pubkey()?.to_string();

        Ok(Self {
            provider,
            wallet_id: wallet_id.to_owned(),
            provider_network,
            provider_chain_id,
            signer_x_only_pubkey,
            state: Mutex::new(CoordinatorState::default()),
        })
    }

    /// Restore a coordinator from one persisted snapshot blob.
    ///
    /// If the snapshot does not match the current provider identity, the coordinator starts empty.
    #[uniffi::constructor(name = "from_snapshot_json")]
    pub fn from_snapshot_json(
        provider: Arc<WalletAbiProvider>,
        wallet_id: &str,
        snapshot_json: &str,
    ) -> Result<Self, LwkError> {
        let coordinator = Self::new(provider, wallet_id)?;
        let snapshot: CoordinatorSnapshot = serde_json::from_str(snapshot_json)?;
        if !coordinator.snapshot_matches(&snapshot) {
            return Ok(coordinator);
        }

        let mut state = coordinator.state.lock()?;
        state.active_sessions = snapshot
            .active_sessions
            .into_iter()
            .map(|session| (session.chain_id.clone(), session))
            .collect();
        state.overlays = snapshot
            .overlays
            .into_iter()
            .map(|overlay| match overlay {
                OverlaySnapshot::Connection(inner) => OverlayState::Connection(inner),
                OverlaySnapshot::Transaction(inner) => OverlayState::Transaction(inner),
            })
            .collect();
        state.pending_actions = snapshot
            .pending_actions
            .into_iter()
            .map(|entry| {
                (
                    entry.action.action_id.clone(),
                    PendingActionEntry {
                        action: entry.action,
                        state: entry.state,
                    },
                )
            })
            .collect();
        state.completed_requests = snapshot
            .completed_requests
            .into_iter()
            .map(|entry| (entry.request_key, entry.outcome))
            .collect();
        state.last_error = snapshot.last_error;

        drop(state);
        Ok(coordinator)
    }

    /// Return the persisted coordinator snapshot blob.
    pub fn snapshot_json(&self) -> Result<String, LwkError> {
        let state = self.state.lock()?;
        let snapshot = CoordinatorSnapshot {
            schema_version: SNAPSHOT_SCHEMA_VERSION,
            wallet_id: self.wallet_id.clone(),
            chain_id: self.provider_chain_id.clone(),
            signer_x_only_pubkey: self.signer_x_only_pubkey.clone(),
            active_sessions: state.active_sessions.values().cloned().collect(),
            overlays: state
                .overlays
                .iter()
                .cloned()
                .map(|overlay| match overlay {
                    OverlayState::Connection(inner) => OverlaySnapshot::Connection(inner),
                    OverlayState::Transaction(inner) => OverlaySnapshot::Transaction(inner),
                })
                .collect(),
            pending_actions: state
                .pending_actions
                .values()
                .cloned()
                .map(|entry| PendingActionSnapshot {
                    action: entry.action,
                    state: entry.state,
                })
                .collect(),
            completed_requests: state
                .completed_requests
                .iter()
                .map(|(request_key, outcome)| CompletedRequestSnapshot {
                    request_key: request_key.clone(),
                    outcome: outcome.clone(),
                })
                .collect(),
            last_error: state.last_error.clone(),
        };

        Ok(serde_json::to_string(&snapshot)?)
    }

    /// Return the current UI-facing state snapshot.
    pub fn ui_state(&self) -> Result<WalletAbiWalletConnectUiState, LwkError> {
        let state = self.state.lock()?;
        let current_overlay = state.overlays.front().map(overlay_to_public);
        let queued_overlay_count = state.overlays.len().saturating_sub(1) as u32;
        Ok(WalletAbiWalletConnectUiState {
            active_sessions: state.active_sessions.values().cloned().collect(),
            current_overlay,
            queued_overlay_count,
            last_error: state.last_error.clone(),
            pending_action_count: state.pending_actions.len() as u32,
        })
    }

    /// Normalize one pairing URI before the app hands it to the WalletConnect SDK.
    pub fn normalize_pairing_uri(&self, input: &str) -> Result<String, LwkError> {
        let normalized = input.trim();
        if !normalized.starts_with("wc:") {
            return Err(LwkError::from(
                "wallet connect pairing URI must start with `wc:`",
            ));
        }
        Ok(normalized.to_owned())
    }

    /// Handle one inbound session proposal.
    pub fn handle_session_proposal(
        &self,
        proposal: WalletAbiWalletConnectSessionProposal,
    ) -> Result<Vec<WalletAbiWalletConnectTransportAction>, LwkError> {
        let mut state = self.state.lock()?;

        if state
            .overlays
            .iter()
            .any(|overlay| matches!(overlay, OverlayState::Connection(inner) if inner.proposal.proposal_id == proposal.proposal_id))
        {
            return Ok(Vec::new());
        }

        match self.validate_proposal(&proposal) {
            Ok(_) => {
                state
                    .overlays
                    .push_back(OverlayState::Connection(ConnectionOverlayState {
                        proposal,
                        awaiting_transport: false,
                        decision: None,
                        pending_action_ids: Vec::new(),
                    }));
                state.last_error = None;
                Ok(Vec::new())
            }
            Err(message) => {
                let message = message.to_string();
                let chain_id = first_requested_chain_id(&proposal)
                    .unwrap_or_else(|| self.provider_chain_id.clone());
                let action = reject_session_action(
                    proposal.proposal_id,
                    &chain_id,
                    WalletAbiWalletConnectReasonKind::UnsupportedProposal,
                    &message,
                );
                insert_pending_action(
                    &mut state,
                    action.clone(),
                    PendingActionState::RejectSession,
                );
                state.last_error = Some(message);
                Ok(vec![action])
            }
        }
    }

    /// Handle one inbound session request.
    pub fn handle_session_request(
        &self,
        request: WalletAbiWalletConnectSessionRequest,
    ) -> Result<Vec<WalletAbiWalletConnectTransportAction>, LwkError> {
        let mut state = self.state.lock()?;
        let request_key = RequestKey::from_request(&request);

        if let Some(existing) = pending_request_action_for_key(&state, &request_key) {
            return Ok(vec![existing.action.clone()]);
        }
        if let Some(action) = duplicate_overlay_action(&mut state, &request_key) {
            return Ok(vec![action]);
        }
        if overlay_contains_request_key(&state, &request_key) {
            return Ok(Vec::new());
        }
        if let Some(outcome) = state.completed_requests.get(&request_key).cloned() {
            let action = action_from_cached_outcome(&request.topic, outcome.clone());
            insert_pending_response_action(&mut state, &request_key, outcome, action.clone());
            return Ok(vec![action]);
        }

        let Some(active_session) = find_active_session(&state, &request) else {
            let action = rpc_error_action(
                &request,
                WalletAbiWalletConnectRpcErrorKind::SessionNotFound,
                SESSION_NOT_FOUND_MESSAGE,
            );
            let outcome = CachedResponseOutcome::RpcError {
                chain_id: request.chain_id.clone(),
                request_id: request.request_id,
                method: request.method.clone(),
                error_kind: WalletAbiWalletConnectRpcErrorKind::SessionNotFound,
                message: SESSION_NOT_FOUND_MESSAGE.to_owned(),
            };
            insert_pending_response_action(&mut state, &request_key, outcome, action.clone());
            return Ok(vec![action]);
        };

        if !SUPPORTED_METHODS.contains(&request.method.as_str()) {
            let action = rpc_error_action(
                &request,
                WalletAbiWalletConnectRpcErrorKind::MethodNotSupported,
                &format!("unsupported wallet-abi method '{}'", request.method),
            );
            let outcome = CachedResponseOutcome::RpcError {
                chain_id: request.chain_id.clone(),
                request_id: request.request_id,
                method: request.method.clone(),
                error_kind: WalletAbiWalletConnectRpcErrorKind::MethodNotSupported,
                message: format!("unsupported wallet-abi method '{}'", request.method),
            };
            insert_pending_response_action(&mut state, &request_key, outcome, action.clone());
            return Ok(vec![action]);
        }

        match request.method.as_str() {
            GET_SIGNER_RECEIVE_ADDRESS_METHOD
            | GET_RAW_SIGNING_X_ONLY_PUBKEY_METHOD
            | WALLET_ABI_GET_CAPABILITIES_METHOD => {
                match self
                    .provider
                    .dispatch_json(&request.method, &request.params_json)
                {
                    Ok(result_json) => {
                        let action = success_response_action(&request, &result_json);
                        let outcome = CachedResponseOutcome::Success {
                            chain_id: request.chain_id.clone(),
                            request_id: request.request_id,
                            method: request.method.clone(),
                            result_json,
                        };
                        insert_pending_response_action(
                            &mut state,
                            &request_key,
                            outcome,
                            action.clone(),
                        );
                        Ok(vec![action])
                    }
                    Err(error) => {
                        let message = error.to_string();
                        let action = rpc_error_action(
                            &request,
                            WalletAbiWalletConnectRpcErrorKind::InvalidRequest,
                            &message,
                        );
                        let outcome = CachedResponseOutcome::RpcError {
                            chain_id: request.chain_id.clone(),
                            request_id: request.request_id,
                            method: request.method.clone(),
                            error_kind: WalletAbiWalletConnectRpcErrorKind::InvalidRequest,
                            message,
                        };
                        insert_pending_response_action(
                            &mut state,
                            &request_key,
                            outcome,
                            action.clone(),
                        );
                        Ok(vec![action])
                    }
                }
            }
            WALLET_ABI_EVALUATE_REQUEST_METHOD => {
                let evaluate_request =
                    match WalletAbiTxEvaluateRequest::from_json(&request.params_json) {
                        Ok(request_obj) => request_obj,
                        Err(error) => {
                            let message = error.to_string();
                            let action = rpc_error_action(
                                &request,
                                WalletAbiWalletConnectRpcErrorKind::InvalidRequest,
                                &message,
                            );
                            let outcome = CachedResponseOutcome::RpcError {
                                chain_id: request.chain_id.clone(),
                                request_id: request.request_id,
                                method: request.method.clone(),
                                error_kind: WalletAbiWalletConnectRpcErrorKind::InvalidRequest,
                                message,
                            };
                            insert_pending_response_action(
                                &mut state,
                                &request_key,
                                outcome,
                                action.clone(),
                            );
                            return Ok(vec![action]);
                        }
                    };

                let result_json = if self.request_network_matches_chain(
                    evaluate_request.network().as_ref(),
                    &request.chain_id,
                ) {
                    self.provider
                        .evaluate_request(evaluate_request.as_ref())?
                        .to_json()?
                } else {
                    create_evaluate_error_response_json(
                        evaluate_request.as_ref(),
                        "request network does not match wallet connect chain",
                    )?
                };

                let action = success_response_action(&request, &result_json);
                let outcome = CachedResponseOutcome::Success {
                    chain_id: request.chain_id.clone(),
                    request_id: request.request_id,
                    method: request.method.clone(),
                    result_json,
                };
                insert_pending_response_action(&mut state, &request_key, outcome, action.clone());
                Ok(vec![action])
            }
            WALLET_ABI_PROCESS_REQUEST_METHOD => {
                let create_request = match WalletAbiTxCreateRequest::from_json(&request.params_json)
                {
                    Ok(request_obj) => request_obj,
                    Err(error) => {
                        let message = error.to_string();
                        let action = rpc_error_action(
                            &request,
                            WalletAbiWalletConnectRpcErrorKind::InvalidRequest,
                            &message,
                        );
                        let outcome = CachedResponseOutcome::RpcError {
                            chain_id: request.chain_id.clone(),
                            request_id: request.request_id,
                            method: request.method.clone(),
                            error_kind: WalletAbiWalletConnectRpcErrorKind::InvalidRequest,
                            message,
                        };
                        insert_pending_response_action(
                            &mut state,
                            &request_key,
                            outcome,
                            action.clone(),
                        );
                        return Ok(vec![action]);
                    }
                };

                if !self.request_network_matches_chain(
                    create_request.network().as_ref(),
                    &request.chain_id,
                ) {
                    let result_json = create_tx_create_error_response_json(
                        create_request.as_ref(),
                        "request network does not match wallet connect chain",
                    )?;
                    let action = success_response_action(&request, &result_json);
                    let outcome = CachedResponseOutcome::Success {
                        chain_id: request.chain_id.clone(),
                        request_id: request.request_id,
                        method: request.method.clone(),
                        result_json,
                    };
                    insert_pending_response_action(
                        &mut state,
                        &request_key,
                        outcome,
                        action.clone(),
                    );
                    return Ok(vec![action]);
                }

                let frozen_session = self.provider.capture_request_session()?;
                let evaluate_request = WalletAbiTxEvaluateRequest::from_parts(
                    &create_request.request_id(),
                    create_request.network().as_ref(),
                    create_request.params().as_ref(),
                )?;
                let evaluate_response = self
                    .provider
                    .evaluate_request_with_session(&frozen_session, evaluate_request.as_ref())?;

                if evaluate_response.preview().is_none() || evaluate_response.error_info().is_some()
                {
                    let result_json = if let Some(error_info) = evaluate_response.error_info() {
                        WalletAbiTxCreateResponse::error(
                            &create_request.request_id(),
                            create_request.network().as_ref(),
                            error_info.as_ref(),
                        )?
                        .to_json()?
                    } else {
                        create_tx_create_error_response_json(
                            create_request.as_ref(),
                            "evaluate_request did not return a preview",
                        )?
                    };
                    let action = success_response_action(&request, &result_json);
                    let outcome = CachedResponseOutcome::Success {
                        chain_id: request.chain_id.clone(),
                        request_id: request.request_id,
                        method: request.method.clone(),
                        result_json,
                    };
                    insert_pending_response_action(
                        &mut state,
                        &request_key,
                        outcome,
                        action.clone(),
                    );
                    return Ok(vec![action]);
                }

                state
                    .overlays
                    .push_back(OverlayState::Transaction(TransactionOverlayState {
                        request,
                        request_json: create_request.to_json()?,
                        preview_json: evaluate_response
                            .preview()
                            .expect("checked preview above")
                            .to_json()?,
                        frozen_session: RequestSessionSnapshot::from_request_session(
                            &frozen_session,
                            &self.provider_chain_id,
                        )?,
                        awaiting_transport: false,
                        decision: None,
                        pending_action_ids: Vec::new(),
                    }));

                debug_assert_eq!(active_session.chain_id, self.provider_chain_id);
                Ok(Vec::new())
            }
            _ => unreachable!("unsupported methods handled above"),
        }
    }

    /// Handle one peer-driven session delete.
    pub fn handle_session_delete(&self, topic: &str) -> Result<(), LwkError> {
        let mut state = self.state.lock()?;
        state
            .active_sessions
            .retain(|_, session| session.topic != topic);
        state.overlays = state
            .overlays
            .drain(..)
            .filter(|overlay| match overlay {
                OverlayState::Connection(_) => true,
                OverlayState::Transaction(inner) => inner.request.topic != topic,
            })
            .collect();
        state.last_error = None;
        Ok(())
    }

    /// Handle one peer-driven session update/extend.
    pub fn handle_session_extend(
        &self,
        session_info: WalletAbiWalletConnectSessionInfo,
    ) -> Result<(), LwkError> {
        let mut state = self.state.lock()?;
        if session_info.chain_id != self.provider_chain_id {
            return Err(LwkError::from(
                "session chain does not match coordinator provider chain",
            ));
        }
        state
            .active_sessions
            .insert(session_info.chain_id.clone(), session_info);
        Ok(())
    }

    /// Approve the current overlay.
    pub fn approve_current_overlay(
        &self,
    ) -> Result<Vec<WalletAbiWalletConnectTransportAction>, LwkError> {
        let mut state = self.state.lock()?;
        let Some(current) = state.overlays.front() else {
            return Err(LwkError::from("no wallet connect overlay to approve"));
        };

        match current.clone() {
            OverlayState::Connection(inner) => {
                if !inner.pending_action_ids.is_empty() {
                    return pending_actions_by_ids(&state, &inner.pending_action_ids);
                }

                let validated = self.validate_proposal(&inner.proposal)?;
                let mut actions = Vec::new();

                if let Some(existing) = state.active_sessions.get(&validated.chain_id).cloned() {
                    let disconnect_action = disconnect_session_action(
                        &existing.topic,
                        &existing.chain_id,
                        WalletAbiWalletConnectReasonKind::ReplacedSession,
                        "wallet connect session replaced by a new approval",
                    );
                    actions.push(disconnect_action.clone());
                    insert_pending_action(
                        &mut state,
                        disconnect_action,
                        PendingActionState::DisconnectSession {
                            chain_id: existing.chain_id,
                            topic: existing.topic,
                        },
                    );
                }

                let approve_action = approve_session_action(
                    inner.proposal.proposal_id,
                    &validated.chain_id,
                    validated.methods,
                    vec![self.wallet_account()],
                );
                insert_pending_action(
                    &mut state,
                    approve_action.clone(),
                    PendingActionState::ApproveSession {
                        chain_id: validated.chain_id,
                        expected_methods: approve_action
                            .approve_session
                            .as_ref()
                            .expect("approve action payload")
                            .methods
                            .clone(),
                        expected_accounts: approve_action
                            .approve_session
                            .as_ref()
                            .expect("approve action payload")
                            .accounts
                            .clone(),
                    },
                );
                actions.push(approve_action);

                let pending_action_ids = actions
                    .iter()
                    .map(|action| action.action_id.clone())
                    .collect::<Vec<_>>();
                let Some(OverlayState::Connection(front)) = state.overlays.front_mut() else {
                    return Err(LwkError::from(
                        "wallet connect overlay changed while approving",
                    ));
                };
                front.awaiting_transport = true;
                front.decision = Some(ConnectionOverlayDecision::Approve);
                front.pending_action_ids = pending_action_ids;
                Ok(actions)
            }
            OverlayState::Transaction(mut inner) => {
                if !inner.pending_action_ids.is_empty() {
                    return pending_actions_by_ids(&state, &inner.pending_action_ids);
                }

                let outcome = match inner.decision.clone() {
                    Some(TransactionOverlayDecision::Approve { outcome }) => outcome,
                    Some(TransactionOverlayDecision::Reject { .. }) => {
                        return Err(LwkError::from(
                            "wallet connect transaction overlay is already prepared for rejection",
                        ))
                    }
                    None => {
                        let request = WalletAbiTxCreateRequest::from_json(&inner.request_json)?;
                        let frozen_session = inner
                            .frozen_session
                            .to_request_session(&self.provider_network)?;
                        let response = self
                            .provider
                            .process_request_with_session(&frozen_session, request.as_ref())?;
                        let result_json = response.to_json()?;
                        let outcome = CachedResponseOutcome::Success {
                            chain_id: inner.request.chain_id.clone(),
                            request_id: inner.request.request_id,
                            method: inner.request.method.clone(),
                            result_json,
                        };
                        inner.decision = Some(TransactionOverlayDecision::Approve {
                            outcome: outcome.clone(),
                        });
                        outcome
                    }
                };

                let action = action_from_cached_outcome(&inner.request.topic, outcome.clone());
                let decision_outcome = outcome.clone();
                insert_pending_response_action(
                    &mut state,
                    &RequestKey::from_request(&inner.request),
                    outcome,
                    action.clone(),
                );
                let Some(OverlayState::Transaction(front)) = state.overlays.front_mut() else {
                    return Err(LwkError::from(
                        "wallet connect overlay changed while approving",
                    ));
                };
                front.awaiting_transport = true;
                front.pending_action_ids = vec![action.action_id.clone()];
                if front.decision.is_none() {
                    front.decision = Some(TransactionOverlayDecision::Approve {
                        outcome: decision_outcome,
                    });
                }
                Ok(vec![action])
            }
        }
    }

    /// Reject the current overlay.
    pub fn reject_current_overlay(
        &self,
    ) -> Result<Vec<WalletAbiWalletConnectTransportAction>, LwkError> {
        let mut state = self.state.lock()?;
        let Some(current) = state.overlays.front() else {
            return Err(LwkError::from("no wallet connect overlay to reject"));
        };

        match current.clone() {
            OverlayState::Connection(inner) => {
                if !inner.pending_action_ids.is_empty() {
                    return Err(LwkError::from(
                        "wallet connect overlay is awaiting transport acknowledgement",
                    ));
                }

                let chain_id = self
                    .validate_proposal(&inner.proposal)
                    .map(|proposal| proposal.chain_id)
                    .unwrap_or_else(|_| {
                        first_requested_chain_id(&inner.proposal)
                            .unwrap_or_else(|| self.provider_chain_id.clone())
                    });
                let action = reject_session_action(
                    inner.proposal.proposal_id,
                    &chain_id,
                    WalletAbiWalletConnectReasonKind::UserRejected,
                    USER_REJECTED_MESSAGE,
                );
                insert_pending_action(
                    &mut state,
                    action.clone(),
                    PendingActionState::RejectSession,
                );
                let Some(OverlayState::Connection(front)) = state.overlays.front_mut() else {
                    return Err(LwkError::from(
                        "wallet connect overlay changed while rejecting",
                    ));
                };
                front.awaiting_transport = true;
                front.decision = Some(ConnectionOverlayDecision::Reject);
                front.pending_action_ids = vec![action.action_id.clone()];
                Ok(vec![action])
            }
            OverlayState::Transaction(mut inner) => {
                if !inner.pending_action_ids.is_empty() {
                    return Err(LwkError::from(
                        "wallet connect overlay is awaiting transport acknowledgement",
                    ));
                }

                let outcome = match inner.decision.clone() {
                    Some(TransactionOverlayDecision::Reject { outcome }) => outcome,
                    Some(TransactionOverlayDecision::Approve { .. }) => {
                        return Err(LwkError::from(
                            "wallet connect transaction overlay already prepared for approval",
                        ))
                    }
                    None => {
                        let request = WalletAbiTxCreateRequest::from_json(&inner.request_json)?;
                        let result_json = create_tx_create_error_response_json(
                            request.as_ref(),
                            USER_REJECTED_MESSAGE,
                        )?;
                        let outcome = CachedResponseOutcome::Success {
                            chain_id: inner.request.chain_id.clone(),
                            request_id: inner.request.request_id,
                            method: inner.request.method.clone(),
                            result_json,
                        };
                        inner.decision = Some(TransactionOverlayDecision::Reject {
                            outcome: outcome.clone(),
                        });
                        outcome
                    }
                };

                let action = action_from_cached_outcome(&inner.request.topic, outcome.clone());
                let decision_outcome = outcome.clone();
                insert_pending_response_action(
                    &mut state,
                    &RequestKey::from_request(&inner.request),
                    outcome,
                    action.clone(),
                );
                let Some(OverlayState::Transaction(front)) = state.overlays.front_mut() else {
                    return Err(LwkError::from(
                        "wallet connect overlay changed while rejecting",
                    ));
                };
                front.awaiting_transport = true;
                front.pending_action_ids = vec![action.action_id.clone()];
                if front.decision.is_none() {
                    front.decision = Some(TransactionOverlayDecision::Reject {
                        outcome: decision_outcome,
                    });
                }
                Ok(vec![action])
            }
        }
    }

    /// Disconnect the active session for one chain.
    pub fn disconnect_active_session(
        &self,
        chain_id: &str,
    ) -> Result<Vec<WalletAbiWalletConnectTransportAction>, LwkError> {
        let mut state = self.state.lock()?;
        let Some(session) = state.active_sessions.get(chain_id).cloned() else {
            return Ok(Vec::new());
        };

        if let Some(existing) = state.pending_actions.values().find(|entry| {
            matches!(
                &entry.state,
                PendingActionState::DisconnectSession {
                    chain_id: pending_chain_id,
                    topic,
                } if pending_chain_id == chain_id && topic == &session.topic
            )
        }) {
            return Ok(vec![existing.action.clone()]);
        }

        let action = disconnect_session_action(
            &session.topic,
            chain_id,
            WalletAbiWalletConnectReasonKind::UserDisconnected,
            "wallet connect session disconnected by user",
        );
        insert_pending_action(
            &mut state,
            action.clone(),
            PendingActionState::DisconnectSession {
                chain_id: chain_id.to_owned(),
                topic: session.topic,
            },
        );
        Ok(vec![action])
    }

    /// Ack one successful session-approval action with the confirmed WalletConnect session info.
    pub fn handle_approve_session_succeeded(
        &self,
        action_id: &str,
        confirmed_session_info: WalletAbiWalletConnectSessionInfo,
    ) -> Result<(), LwkError> {
        let mut state = self.state.lock()?;
        let Some(entry) = state.pending_actions.remove(action_id) else {
            return Err(LwkError::from("unknown transport action id"));
        };

        let PendingActionState::ApproveSession {
            chain_id,
            expected_methods,
            expected_accounts,
        } = entry.state
        else {
            return Err(LwkError::from(
                "approve session success ack must target an approve-session action",
            ));
        };

        if confirmed_session_info.chain_id != chain_id {
            return Err(LwkError::from(
                "confirmed session chain does not match approved chain",
            ));
        }
        if dedup_strings(confirmed_session_info.methods.clone()) != dedup_strings(expected_methods)
        {
            return Err(LwkError::from(
                "confirmed session methods do not match the approved method set",
            ));
        }
        if dedup_strings(confirmed_session_info.accounts.clone())
            != dedup_strings(expected_accounts)
        {
            return Err(LwkError::from(
                "confirmed session accounts do not match the approved account set",
            ));
        }

        state
            .active_sessions
            .insert(chain_id.clone(), confirmed_session_info);
        finish_overlay_action(&mut state, action_id, true);
        Ok(())
    }

    /// Ack one successful non-approval transport action.
    pub fn handle_transport_action_succeeded(&self, action_id: &str) -> Result<(), LwkError> {
        let mut state = self.state.lock()?;
        apply_transport_action_success(&mut state, action_id)
    }

    /// Ack one failed transport action.
    pub fn handle_transport_action_failed(
        &self,
        action_id: &str,
        message: &str,
    ) -> Result<(), LwkError> {
        let mut state = self.state.lock()?;
        if !state.pending_actions.contains_key(action_id) {
            return Err(LwkError::from("unknown transport action id"));
        }

        state.pending_actions.remove(action_id);
        fail_overlay_action(&mut state, action_id);
        state.last_error = Some(message.to_owned());
        Ok(())
    }

    /// Reconcile active sessions on cold start and re-emit any still-pending session actions.
    pub fn reconcile_active_sessions(
        &self,
        active_sessions: Vec<WalletAbiWalletConnectSessionInfo>,
    ) -> Result<Vec<WalletAbiWalletConnectTransportAction>, LwkError> {
        let mut state = self.state.lock()?;
        let mut normalized = BTreeMap::new();
        let mut actions = Vec::new();
        for session in active_sessions {
            if session.chain_id != self.provider_chain_id {
                continue;
            }
            normalized.insert(session.chain_id.clone(), session);
        }

        let pending_ids: Vec<String> = state.pending_actions.keys().cloned().collect();
        for action_id in pending_ids {
            let Some(entry) = state.pending_actions.get(&action_id).cloned() else {
                continue;
            };
            match &entry.state {
                PendingActionState::ApproveSession {
                    chain_id,
                    expected_methods,
                    expected_accounts,
                } => {
                    if let Some(session) = normalized.get(chain_id).cloned() {
                        if dedup_strings(session.methods.clone())
                            == dedup_strings(expected_methods.clone())
                            && dedup_strings(session.accounts.clone())
                                == dedup_strings(expected_accounts.clone())
                        {
                            state.pending_actions.remove(&action_id);
                            state.active_sessions.insert(chain_id.clone(), session);
                            finish_overlay_action(&mut state, &action_id, true);
                        } else {
                            actions.push(entry.action.clone());
                        }
                    } else {
                        actions.push(entry.action.clone());
                    }
                }
                PendingActionState::DisconnectSession { topic, .. } => {
                    if normalized.values().all(|session| session.topic != *topic) {
                        apply_transport_action_success(&mut state, &action_id)?;
                    } else {
                        actions.push(entry.action.clone());
                    }
                }
                PendingActionState::RejectSession => {
                    apply_transport_action_success(&mut state, &action_id)?;
                }
                PendingActionState::RespondSuccess { .. }
                | PendingActionState::RespondWalletAbiError { .. } => {}
            }
        }

        state.active_sessions = normalized;
        Ok(actions)
    }

    /// Reconcile pending requests on cold start and surface any requests the app must replay.
    pub fn reconcile_pending_requests(
        &self,
        pending_requests: Vec<WalletAbiWalletConnectSessionRequest>,
    ) -> Result<WalletAbiWalletConnectPendingRequestsReconcileResult, LwkError> {
        let mut state = self.state.lock()?;
        let request_keys: BTreeSet<RequestKey> = pending_requests
            .iter()
            .map(RequestKey::from_request)
            .collect();

        let pending_response_ids: Vec<String> = state
            .pending_actions
            .iter()
            .filter_map(|(action_id, entry)| match &entry.state {
                PendingActionState::RespondSuccess { request_key, .. }
                | PendingActionState::RespondWalletAbiError { request_key, .. } => {
                    if !request_keys.contains(request_key) {
                        Some(action_id.clone())
                    } else {
                        None
                    }
                }
                _ => None,
            })
            .collect();

        for action_id in pending_response_ids {
            apply_transport_action_success(&mut state, &action_id)?;
        }

        let mut actions = Vec::new();
        let mut requests_to_replay = Vec::new();

        for request in pending_requests {
            let request_key = RequestKey::from_request(&request);

            if let Some(existing) = pending_request_action_for_key(&state, &request_key) {
                actions.push(existing.action.clone());
                continue;
            }
            if let Some(action) = duplicate_overlay_action(&mut state, &request_key) {
                actions.push(action);
                continue;
            }
            if overlay_contains_request_key(&state, &request_key) {
                continue;
            }
            if let Some(outcome) = state.completed_requests.get(&request_key).cloned() {
                let action = action_from_cached_outcome(&request.topic, outcome.clone());
                insert_pending_response_action(&mut state, &request_key, outcome, action.clone());
                actions.push(action);
                continue;
            }

            requests_to_replay.push(request);
        }

        Ok(WalletAbiWalletConnectPendingRequestsReconcileResult {
            actions,
            requests_to_replay,
        })
    }

    /// Clear the last surfaced coordinator error.
    pub fn clear_error(&self) -> Result<(), LwkError> {
        let mut state = self.state.lock()?;
        state.last_error = None;
        Ok(())
    }
}

impl WalletAbiWalletConnectCoordinator {
    fn snapshot_matches(&self, snapshot: &CoordinatorSnapshot) -> bool {
        snapshot.schema_version == SNAPSHOT_SCHEMA_VERSION
            && snapshot.wallet_id == self.wallet_id
            && snapshot.chain_id == self.provider_chain_id
            && snapshot.signer_x_only_pubkey == self.signer_x_only_pubkey
    }

    fn validate_proposal(
        &self,
        proposal: &WalletAbiWalletConnectSessionProposal,
    ) -> Result<ValidatedProposal, LwkError> {
        let requested_events = dedup_strings(
            proposal
                .required_events
                .iter()
                .chain(&proposal.optional_events)
                .cloned()
                .collect(),
        );
        if !requested_events.is_empty() {
            return Err(LwkError::from(
                "wallet connect wallet-abi proposals must not request events",
            ));
        }

        let chain_ids = dedup_strings(
            proposal
                .required_chain_ids
                .iter()
                .chain(&proposal.optional_chain_ids)
                .cloned()
                .collect(),
        );
        if chain_ids.is_empty() {
            return Err(LwkError::from(
                "wallet connect wallet-abi proposal must request at least one chain",
            ));
        }
        if chain_ids != vec![self.provider_chain_id.clone()] {
            return Err(LwkError::from(format!(
                "wallet connect wallet-abi proposals must target only '{}'",
                self.provider_chain_id
            )));
        }

        let methods = dedup_strings(
            proposal
                .required_methods
                .iter()
                .chain(&proposal.optional_methods)
                .cloned()
                .collect(),
        );
        if methods.is_empty() {
            return Err(LwkError::from(
                "wallet connect wallet-abi proposal must request at least one method",
            ));
        }
        let unsupported: Vec<String> = methods
            .iter()
            .filter(|method| !SUPPORTED_METHODS.contains(&method.as_str()))
            .cloned()
            .collect();
        if !unsupported.is_empty() {
            return Err(LwkError::from(format!(
                "unsupported wallet-abi methods requested: {}",
                unsupported.join(", ")
            )));
        }

        Ok(ValidatedProposal {
            chain_id: self.provider_chain_id.clone(),
            methods,
        })
    }

    fn wallet_account(&self) -> String {
        format!("{}:{}", self.provider_chain_id, self.wallet_id)
    }

    fn request_network_matches_chain(&self, network: &Network, chain_id: &str) -> bool {
        network_to_wallet_connect_chain(network)
            .map(|derived| derived == chain_id)
            .unwrap_or(false)
    }
}

impl RequestKey {
    fn from_request(request: &WalletAbiWalletConnectSessionRequest) -> Self {
        Self {
            topic: request.topic.clone(),
            request_id: request.request_id,
            method: request.method.clone(),
        }
    }
}

impl RequestSessionSnapshot {
    fn from_request_session(
        session: &WalletAbiRequestSession,
        chain_id: &str,
    ) -> Result<Self, LwkError> {
        let spendable_utxos = session
            .spendable_utxos
            .iter()
            .map(|utxo| {
                let inner = utxo.inner();
                Ok(ExternalUtxoSnapshot {
                    outpoint: inner.outpoint.to_string(),
                    txout_hex: serialize(&inner.txout).to_hex(),
                    tx_hex: inner.tx.as_ref().map(|tx| serialize(tx).to_hex()),
                    asset_id: inner.unblinded.asset.to_string(),
                    asset_bf: inner.unblinded.asset_bf.to_string(),
                    value: inner.unblinded.value,
                    value_bf: inner.unblinded.value_bf.to_string(),
                    max_weight_to_satisfy: inner.max_weight_to_satisfy as u32,
                })
            })
            .collect::<Result<Vec<_>, LwkError>>()?;

        Ok(Self {
            session_id: session.session_id.clone(),
            chain_id: chain_id.to_owned(),
            spendable_utxos,
        })
    }

    fn to_request_session(
        &self,
        network: &Arc<Network>,
    ) -> Result<WalletAbiRequestSession, LwkError> {
        let spendable_utxos = self
            .spendable_utxos
            .iter()
            .map(ExternalUtxoSnapshot::to_external_utxo)
            .collect::<Result<Vec<_>, LwkError>>()?;
        Ok(WalletAbiRequestSession {
            session_id: self.session_id.clone(),
            network: network.clone(),
            spendable_utxos,
        })
    }
}

impl ExternalUtxoSnapshot {
    fn to_external_utxo(&self) -> Result<Arc<ExternalUtxo>, LwkError> {
        let outpoint = OutPoint::new(&self.outpoint)?;
        let txout = TxOut::from(deserialize::<elements::TxOut>(&Vec::<u8>::from_hex(
            &self.txout_hex,
        )?)?);
        let tx = match &self.tx_hex {
            Some(tx_hex) => Some(Transaction::from(deserialize::<elements::Transaction>(
                &Vec::<u8>::from_hex(tx_hex)?,
            )?)),
            None => None,
        };
        let asset_id: AssetId = elements::AssetId::from_str(&self.asset_id)?.into();
        let asset_bf = AssetBlindingFactor::from_str(&self.asset_bf)?;
        let value_bf = ValueBlindingFactor::from_str(&self.value_bf)?;
        let secrets = TxOutSecrets::new(asset_id, &asset_bf, self.value, &value_bf);
        let utxo = lwk_wollet::ExternalUtxo {
            outpoint: outpoint.as_ref().into(),
            txout: txout.as_ref().clone(),
            tx: tx.map(|tx| tx.into()),
            unblinded: secrets.as_ref().into(),
            max_weight_to_satisfy: self.max_weight_to_satisfy as usize,
        };
        Ok(Arc::new(utxo.into()))
    }
}

fn pending_request_action_for_key<'a>(
    state: &'a CoordinatorState,
    request_key: &RequestKey,
) -> Option<&'a PendingActionEntry> {
    state
        .pending_actions
        .values()
        .find(|entry| match &entry.state {
            PendingActionState::RespondSuccess {
                request_key: key, ..
            }
            | PendingActionState::RespondWalletAbiError {
                request_key: key, ..
            } => key == request_key,
            _ => false,
        })
}

fn duplicate_overlay_action(
    state: &mut CoordinatorState,
    request_key: &RequestKey,
) -> Option<WalletAbiWalletConnectTransportAction> {
    for overlay in state.overlays.iter_mut() {
        let OverlayState::Transaction(inner) = overlay else {
            continue;
        };
        if RequestKey::from_request(&inner.request) != *request_key {
            continue;
        }
        if let Some(action_id) = inner.pending_action_ids.first() {
            if let Some(entry) = state.pending_actions.get(action_id) {
                return Some(entry.action.clone());
            }
        }
        let outcome = match inner.decision.clone() {
            Some(TransactionOverlayDecision::Approve { outcome })
            | Some(TransactionOverlayDecision::Reject { outcome }) => outcome,
            None => return None,
        };
        let action = action_from_cached_outcome(&inner.request.topic, outcome.clone());
        inner.awaiting_transport = true;
        inner.pending_action_ids = vec![action.action_id.clone()];
        insert_pending_response_action(state, request_key, outcome, action.clone());
        return Some(action);
    }
    None
}

fn overlay_contains_request_key(state: &CoordinatorState, request_key: &RequestKey) -> bool {
    state.overlays.iter().any(|overlay| {
        matches!(
            overlay,
            OverlayState::Transaction(inner) if RequestKey::from_request(&inner.request) == *request_key
        )
    })
}

fn pending_actions_by_ids(
    state: &CoordinatorState,
    action_ids: &[String],
) -> Result<Vec<WalletAbiWalletConnectTransportAction>, LwkError> {
    action_ids
        .iter()
        .map(|action_id| {
            state
                .pending_actions
                .get(action_id)
                .map(|entry| entry.action.clone())
                .ok_or_else(|| {
                    LwkError::from("wallet connect overlay lost a pending transport action")
                })
        })
        .collect()
}

fn insert_pending_action(
    state: &mut CoordinatorState,
    action: WalletAbiWalletConnectTransportAction,
    action_state: PendingActionState,
) {
    state.pending_actions.insert(
        action.action_id.clone(),
        PendingActionEntry {
            action,
            state: action_state,
        },
    );
}

fn insert_pending_response_action(
    state: &mut CoordinatorState,
    request_key: &RequestKey,
    outcome: CachedResponseOutcome,
    action: WalletAbiWalletConnectTransportAction,
) {
    let action_state = match action.kind {
        WalletAbiWalletConnectTransportActionKind::RespondSuccess => {
            PendingActionState::RespondSuccess {
                request_key: request_key.clone(),
                outcome,
            }
        }
        WalletAbiWalletConnectTransportActionKind::RespondWalletAbiError => {
            PendingActionState::RespondWalletAbiError {
                request_key: request_key.clone(),
                outcome,
            }
        }
        _ => unreachable!("response action kind expected"),
    };
    insert_pending_action(state, action, action_state);
}

fn apply_transport_action_success(
    state: &mut CoordinatorState,
    action_id: &str,
) -> Result<(), LwkError> {
    let Some(entry) = state.pending_actions.remove(action_id) else {
        return Err(LwkError::from("unknown transport action id"));
    };

    match entry.state {
        PendingActionState::ApproveSession { .. } => Err(LwkError::from(
            "use handle_approve_session_succeeded for approve actions",
        )),
        PendingActionState::RejectSession => {
            finish_overlay_action(state, action_id, true);
            Ok(())
        }
        PendingActionState::RespondSuccess {
            request_key,
            outcome,
        }
        | PendingActionState::RespondWalletAbiError {
            request_key,
            outcome,
        } => {
            state.completed_requests.insert(request_key, outcome);
            finish_overlay_action(state, action_id, true);
            Ok(())
        }
        PendingActionState::DisconnectSession { chain_id, topic } => {
            if matches!(
                state.active_sessions.get(&chain_id),
                Some(session) if session.topic == topic
            ) {
                state.active_sessions.remove(&chain_id);
            }
            finish_overlay_action(state, action_id, true);
            Ok(())
        }
    }
}

fn finish_overlay_action(state: &mut CoordinatorState, action_id: &str, success: bool) {
    let mut should_pop_front = false;
    if let Some(front) = state.overlays.front_mut() {
        match front {
            OverlayState::Connection(inner) => {
                if remove_pending_action_id(&mut inner.pending_action_ids, action_id) {
                    inner.awaiting_transport = !inner.pending_action_ids.is_empty();
                    if success && inner.pending_action_ids.is_empty() && inner.decision.is_some() {
                        should_pop_front = true;
                    }
                }
            }
            OverlayState::Transaction(inner) => {
                if remove_pending_action_id(&mut inner.pending_action_ids, action_id) {
                    inner.awaiting_transport = !inner.pending_action_ids.is_empty();
                    if success && inner.pending_action_ids.is_empty() && inner.decision.is_some() {
                        should_pop_front = true;
                    }
                }
            }
        }
    }
    if should_pop_front {
        state.overlays.pop_front();
    }
}

fn fail_overlay_action(state: &mut CoordinatorState, action_id: &str) {
    if let Some(front) = state.overlays.front_mut() {
        match front {
            OverlayState::Connection(inner) => {
                if remove_pending_action_id(&mut inner.pending_action_ids, action_id) {
                    inner.awaiting_transport = !inner.pending_action_ids.is_empty();
                }
            }
            OverlayState::Transaction(inner) => {
                if remove_pending_action_id(&mut inner.pending_action_ids, action_id) {
                    inner.awaiting_transport = !inner.pending_action_ids.is_empty();
                }
            }
        }
    }
}

fn remove_pending_action_id(ids: &mut Vec<String>, action_id: &str) -> bool {
    let before = ids.len();
    ids.retain(|id| id != action_id);
    before != ids.len()
}

fn overlay_to_public(overlay: &OverlayState) -> WalletAbiWalletConnectOverlay {
    match overlay {
        OverlayState::Connection(inner) => WalletAbiWalletConnectOverlay {
            kind: WalletAbiWalletConnectOverlayKind::ConnectionApproval,
            chain_id: first_requested_chain_id(&inner.proposal).unwrap_or_default(),
            session_topic: None,
            proposal: Some(inner.proposal.clone()),
            request: None,
            request_json: None,
            preview_json: None,
            awaiting_transport: inner.awaiting_transport,
        },
        OverlayState::Transaction(inner) => WalletAbiWalletConnectOverlay {
            kind: WalletAbiWalletConnectOverlayKind::TransactionApproval,
            chain_id: inner.request.chain_id.clone(),
            session_topic: Some(inner.request.topic.clone()),
            proposal: None,
            request: Some(inner.request.clone()),
            request_json: Some(inner.request_json.clone()),
            preview_json: Some(inner.preview_json.clone()),
            awaiting_transport: inner.awaiting_transport,
        },
    }
}

fn find_active_session(
    state: &CoordinatorState,
    request: &WalletAbiWalletConnectSessionRequest,
) -> Option<WalletAbiWalletConnectSessionInfo> {
    state
        .active_sessions
        .get(&request.chain_id)
        .filter(|session| session.topic == request.topic)
        .cloned()
}

fn dedup_strings(values: Vec<String>) -> Vec<String> {
    let mut seen = BTreeSet::new();
    let mut deduped = Vec::new();
    for value in values {
        if seen.insert(value.clone()) {
            deduped.push(value);
        }
    }
    deduped
}

fn first_requested_chain_id(proposal: &WalletAbiWalletConnectSessionProposal) -> Option<String> {
    proposal
        .required_chain_ids
        .first()
        .cloned()
        .or_else(|| proposal.optional_chain_ids.first().cloned())
}

fn network_to_wallet_connect_chain(network: &Network) -> Result<String, LwkError> {
    match network.inner {
        lwk_wollet::ElementsNetwork::Liquid => {
            Ok(WALLET_ABI_WALLETCONNECT_CHAIN_MAINNET.to_owned())
        }
        lwk_wollet::ElementsNetwork::LiquidTestnet => {
            Ok(WALLET_ABI_WALLETCONNECT_CHAIN_TESTNET.to_owned())
        }
        lwk_wollet::ElementsNetwork::ElementsRegtest { .. } => {
            Ok(WALLET_ABI_WALLETCONNECT_CHAIN_REGTEST.to_owned())
        }
    }
}

fn create_action_id() -> String {
    abi::generate_request_id().to_string()
}

fn approve_session_action(
    proposal_id: u64,
    chain_id: &str,
    methods: Vec<String>,
    accounts: Vec<String>,
) -> WalletAbiWalletConnectTransportAction {
    WalletAbiWalletConnectTransportAction {
        action_id: create_action_id(),
        kind: WalletAbiWalletConnectTransportActionKind::ApproveSession,
        approve_session: Some(WalletAbiWalletConnectApproveSessionAction {
            proposal_id,
            chain_id: chain_id.to_owned(),
            methods,
            accounts,
        }),
        reject_session: None,
        respond_success: None,
        respond_wallet_abi_error: None,
        disconnect_session: None,
    }
}

fn reject_session_action(
    proposal_id: u64,
    chain_id: &str,
    reason_kind: WalletAbiWalletConnectReasonKind,
    message: &str,
) -> WalletAbiWalletConnectTransportAction {
    WalletAbiWalletConnectTransportAction {
        action_id: create_action_id(),
        kind: WalletAbiWalletConnectTransportActionKind::RejectSession,
        approve_session: None,
        reject_session: Some(WalletAbiWalletConnectRejectSessionAction {
            proposal_id,
            chain_id: chain_id.to_owned(),
            reason_kind,
            message: message.to_owned(),
        }),
        respond_success: None,
        respond_wallet_abi_error: None,
        disconnect_session: None,
    }
}

fn success_response_action(
    request: &WalletAbiWalletConnectSessionRequest,
    result_json: &str,
) -> WalletAbiWalletConnectTransportAction {
    WalletAbiWalletConnectTransportAction {
        action_id: create_action_id(),
        kind: WalletAbiWalletConnectTransportActionKind::RespondSuccess,
        approve_session: None,
        reject_session: None,
        respond_success: Some(WalletAbiWalletConnectRespondSuccessAction {
            topic: request.topic.clone(),
            chain_id: request.chain_id.clone(),
            request_id: request.request_id,
            method: request.method.clone(),
            result_json: result_json.to_owned(),
        }),
        respond_wallet_abi_error: None,
        disconnect_session: None,
    }
}

fn rpc_error_action(
    request: &WalletAbiWalletConnectSessionRequest,
    error_kind: WalletAbiWalletConnectRpcErrorKind,
    message: &str,
) -> WalletAbiWalletConnectTransportAction {
    WalletAbiWalletConnectTransportAction {
        action_id: create_action_id(),
        kind: WalletAbiWalletConnectTransportActionKind::RespondWalletAbiError,
        approve_session: None,
        reject_session: None,
        respond_success: None,
        respond_wallet_abi_error: Some(WalletAbiWalletConnectRespondWalletAbiErrorAction {
            topic: request.topic.clone(),
            chain_id: request.chain_id.clone(),
            request_id: request.request_id,
            method: request.method.clone(),
            error_kind,
            message: message.to_owned(),
        }),
        disconnect_session: None,
    }
}

fn disconnect_session_action(
    topic: &str,
    chain_id: &str,
    reason_kind: WalletAbiWalletConnectReasonKind,
    message: &str,
) -> WalletAbiWalletConnectTransportAction {
    WalletAbiWalletConnectTransportAction {
        action_id: create_action_id(),
        kind: WalletAbiWalletConnectTransportActionKind::DisconnectSession,
        approve_session: None,
        reject_session: None,
        respond_success: None,
        respond_wallet_abi_error: None,
        disconnect_session: Some(WalletAbiWalletConnectDisconnectSessionAction {
            topic: topic.to_owned(),
            chain_id: chain_id.to_owned(),
            reason_kind,
            message: message.to_owned(),
        }),
    }
}

fn action_from_cached_outcome(
    topic: &str,
    outcome: CachedResponseOutcome,
) -> WalletAbiWalletConnectTransportAction {
    match outcome {
        CachedResponseOutcome::Success {
            chain_id,
            request_id,
            method,
            result_json,
        } => WalletAbiWalletConnectTransportAction {
            action_id: create_action_id(),
            kind: WalletAbiWalletConnectTransportActionKind::RespondSuccess,
            approve_session: None,
            reject_session: None,
            respond_success: Some(WalletAbiWalletConnectRespondSuccessAction {
                topic: topic.to_owned(),
                chain_id,
                request_id,
                method,
                result_json,
            }),
            respond_wallet_abi_error: None,
            disconnect_session: None,
        },
        CachedResponseOutcome::RpcError {
            chain_id,
            request_id,
            method,
            error_kind,
            message,
        } => WalletAbiWalletConnectTransportAction {
            action_id: create_action_id(),
            kind: WalletAbiWalletConnectTransportActionKind::RespondWalletAbiError,
            approve_session: None,
            reject_session: None,
            respond_success: None,
            respond_wallet_abi_error: Some(WalletAbiWalletConnectRespondWalletAbiErrorAction {
                topic: topic.to_owned(),
                chain_id,
                request_id,
                method,
                error_kind,
                message,
            }),
            disconnect_session: None,
        },
    }
}

fn create_wallet_abi_error_info(message: &str) -> Result<Arc<WalletAbiErrorInfo>, LwkError> {
    WalletAbiErrorInfo::from_code_string("invalid_request", message, None)
}

fn create_tx_create_error_response_json(
    request: &WalletAbiTxCreateRequest,
    message: &str,
) -> Result<String, LwkError> {
    let error_info = create_wallet_abi_error_info(message)?;
    WalletAbiTxCreateResponse::error(
        &request.request_id(),
        request.network().as_ref(),
        error_info.as_ref(),
    )?
    .to_json()
}

fn create_evaluate_error_response_json(
    request: &WalletAbiTxEvaluateRequest,
    message: &str,
) -> Result<String, LwkError> {
    let error_info = create_wallet_abi_error_info(message)?;
    WalletAbiTxEvaluateResponse::error(request, error_info.as_ref()).to_json()
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};

    use super::*;
    use crate::{
        wallet_abi_bip32_derivation_pair_from_signer, wallet_abi_output_template_from_address,
        Address, Mnemonic, OutPoint, Pset, Signer, SignerMetaLink, TxSequence,
        WalletAbiAmountFilter, WalletAbiAssetFilter, WalletAbiAssetVariant,
        WalletAbiBip32DerivationPair, WalletAbiBlinderVariant, WalletAbiBroadcasterCallbacks,
        WalletAbiFinalizerSpec, WalletAbiInputSchema, WalletAbiInputUnblinding,
        WalletAbiLockFilter, WalletAbiLockVariant, WalletAbiOutputAllocatorCallbacks,
        WalletAbiOutputSchema, WalletAbiPrevoutResolverCallbacks,
        WalletAbiReceiveAddressProviderCallbacks, WalletAbiRuntimeParams,
        WalletAbiSessionFactoryCallbacks, WalletAbiSignerCallbacks, WalletAbiUtxoSource,
        WalletAbiWalletOutputRequest, WalletAbiWalletOutputRole, WalletAbiWalletOutputTemplate,
        WalletAbiWalletSourceFilter, WalletBroadcasterLink, WalletOutputAllocatorLink,
        WalletPrevoutResolverLink, WalletReceiveAddressProviderLink, WalletRuntimeDepsLink,
        WalletSessionFactoryLink, XOnlyPublicKey,
    };

    struct TestSignerCallbacks {
        signer: Arc<Signer>,
        expected_xonly: Arc<XOnlyPublicKey>,
    }

    impl WalletAbiSignerCallbacks for TestSignerCallbacks {
        fn get_raw_signing_x_only_pubkey(&self) -> Result<Arc<XOnlyPublicKey>, LwkError> {
            Ok(self.expected_xonly.clone())
        }

        fn sign_pst(&self, pst: Arc<Pset>) -> Result<Arc<Pset>, LwkError> {
            self.signer.sign(pst.as_ref())
        }

        fn sign_schnorr(&self, _message: Vec<u8>) -> Result<Vec<u8>, LwkError> {
            Ok(vec![0; 64])
        }
    }

    struct TestSessionFactoryCallbacks {
        open_calls: Arc<AtomicUsize>,
        session: WalletAbiRequestSession,
    }

    impl WalletAbiSessionFactoryCallbacks for TestSessionFactoryCallbacks {
        fn open_wallet_request_session(&self) -> Result<WalletAbiRequestSession, LwkError> {
            self.open_calls.fetch_add(1, Ordering::SeqCst);
            Ok(self.session.clone())
        }
    }

    struct TestOutputAllocatorCallbacks {
        session_ids: Arc<Mutex<Vec<String>>>,
        address: Arc<Address>,
    }

    impl WalletAbiOutputAllocatorCallbacks for TestOutputAllocatorCallbacks {
        fn get_wallet_output_template(
            &self,
            session: WalletAbiRequestSession,
            request: WalletAbiWalletOutputRequest,
        ) -> Result<WalletAbiWalletOutputTemplate, LwkError> {
            self.session_ids.lock()?.push(session.session_id);
            assert!(matches!(
                request.role,
                WalletAbiWalletOutputRole::Receive | WalletAbiWalletOutputRole::Change
            ));
            Ok(wallet_abi_output_template_from_address(
                self.address.as_ref(),
            ))
        }
    }

    struct TestPrevoutResolverCallbacks {
        derivation_pair: WalletAbiBip32DerivationPair,
        tx_out: Arc<TxOut>,
        tx_out_secrets: Arc<TxOutSecrets>,
    }

    impl WalletAbiPrevoutResolverCallbacks for TestPrevoutResolverCallbacks {
        fn get_bip32_derivation_pair(
            &self,
            _outpoint: Arc<OutPoint>,
        ) -> Result<Option<WalletAbiBip32DerivationPair>, LwkError> {
            Ok(Some(self.derivation_pair.clone()))
        }

        fn unblind(&self, _tx_out: Arc<TxOut>) -> Result<Arc<TxOutSecrets>, LwkError> {
            Ok(self.tx_out_secrets.clone())
        }

        fn get_tx_out(&self, _outpoint: Arc<OutPoint>) -> Result<Arc<TxOut>, LwkError> {
            Ok(self.tx_out.clone())
        }
    }

    struct TestBroadcasterCallbacks {
        broadcast_calls: Arc<AtomicUsize>,
    }

    impl WalletAbiBroadcasterCallbacks for TestBroadcasterCallbacks {
        fn broadcast_transaction(
            &self,
            tx: Arc<Transaction>,
        ) -> Result<Arc<crate::Txid>, LwkError> {
            self.broadcast_calls.fetch_add(1, Ordering::SeqCst);
            Ok(tx.txid())
        }
    }

    struct TestReceiveAddressProviderCallbacks {
        address: Arc<Address>,
    }

    impl WalletAbiReceiveAddressProviderCallbacks for TestReceiveAddressProviderCallbacks {
        fn get_signer_receive_address(&self) -> Result<Arc<Address>, LwkError> {
            Ok(self.address.clone())
        }
    }

    struct TestContext {
        provider: Arc<WalletAbiProvider>,
        external_address: Arc<Address>,
        network: Arc<Network>,
        open_calls: Arc<AtomicUsize>,
        output_session_ids: Arc<Mutex<Vec<String>>>,
        broadcast_calls: Arc<AtomicUsize>,
    }

    fn test_context() -> TestContext {
        let network = Network::testnet();
        let mnemonic = Mnemonic::new(
            "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about",
        )
        .expect("mnemonic");
        let signer = Signer::new(&mnemonic, network.as_ref()).expect("signer");
        let wallet_descriptor = signer.wpkh_slip77_descriptor().expect("descriptor");
        let wallet =
            crate::Wollet::new(network.as_ref(), wallet_descriptor.as_ref(), None).expect("wallet");
        let wallet_address = wallet.address(Some(0)).expect("address result").address();
        let external_address = Address::new(
            "tlq1qq02egjncr8g4qn890mrw3jhgupwqymekv383lwpmsfghn36hac5ptpmeewtnftluqyaraa56ung7wf47crkn5fjuhk422d68m",
        )
        .expect("external address");
        let expected_xonly =
            crate::simplicity_derive_xonly_pubkey(signer.as_ref(), "m/86h/1h/0h/0/0")
                .expect("xonly");
        let derivation_pair = wallet_abi_bip32_derivation_pair_from_signer(
            signer.as_ref(),
            vec![2147483732, 2147483649, 2147483648, 0, 0],
        )
        .expect("derivation pair");
        let policy_asset = network.policy_asset();
        let outpoint = OutPoint::from_parts(
            &crate::Txid::from_string(
                "0000000000000000000000000000000000000000000000000000000000000001",
            )
            .expect("txid"),
            0,
        );
        let tx_out = TxOut::from_explicit(
            wallet_address.script_pubkey().as_ref(),
            policy_asset,
            20_000,
        );
        let tx_out_secrets = TxOutSecrets::from_explicit(policy_asset, 20_000);
        let utxo = ExternalUtxo::from_unchecked_data(&outpoint, &tx_out, &tx_out_secrets, 107);
        let request_session = WalletAbiRequestSession {
            session_id: "frozen-session".to_owned(),
            network: network.clone(),
            spendable_utxos: vec![utxo],
        };

        let open_calls = Arc::new(AtomicUsize::new(0));
        let output_session_ids = Arc::new(Mutex::new(Vec::new()));
        let broadcast_calls = Arc::new(AtomicUsize::new(0));

        let signer_link = Arc::new(SignerMetaLink::new(Arc::new(TestSignerCallbacks {
            signer: signer.clone(),
            expected_xonly,
        })));
        let session_factory = Arc::new(WalletSessionFactoryLink::new(Arc::new(
            TestSessionFactoryCallbacks {
                open_calls: open_calls.clone(),
                session: request_session,
            },
        )));
        let output_allocator = Arc::new(WalletOutputAllocatorLink::new(Arc::new(
            TestOutputAllocatorCallbacks {
                session_ids: output_session_ids.clone(),
                address: wallet_address.clone(),
            },
        )));
        let prevout_resolver = Arc::new(WalletPrevoutResolverLink::new(Arc::new(
            TestPrevoutResolverCallbacks {
                derivation_pair,
                tx_out: tx_out.clone(),
                tx_out_secrets: tx_out_secrets.clone(),
            },
        )));
        let broadcaster = Arc::new(WalletBroadcasterLink::new(Arc::new(
            TestBroadcasterCallbacks {
                broadcast_calls: broadcast_calls.clone(),
            },
        )));
        let receive_address_provider = Arc::new(WalletReceiveAddressProviderLink::new(Arc::new(
            TestReceiveAddressProviderCallbacks {
                address: wallet_address.clone(),
            },
        )));
        let runtime_deps = Arc::new(WalletRuntimeDepsLink::new(
            session_factory,
            output_allocator,
            prevout_resolver,
            broadcaster,
            receive_address_provider,
        ));

        TestContext {
            provider: Arc::new(WalletAbiProvider::new(signer_link, runtime_deps)),
            external_address,
            network,
            open_calls,
            output_session_ids,
            broadcast_calls,
        }
    }

    fn create_request(context: &TestContext) -> Arc<WalletAbiTxCreateRequest> {
        let policy_asset = context.network.policy_asset();
        let asset_filter = WalletAbiAssetFilter::exact(policy_asset);
        let amount_filter = WalletAbiAmountFilter::exact(20_000);
        let lock_filter = WalletAbiLockFilter::none();
        let wallet_filter = WalletAbiWalletSourceFilter::with_filters(
            asset_filter.as_ref(),
            amount_filter.as_ref(),
            lock_filter.as_ref(),
        );
        let utxo_source = WalletAbiUtxoSource::wallet(wallet_filter.as_ref());
        let unblinding = WalletAbiInputUnblinding::wallet();
        let sequence = TxSequence::max();
        let finalizer = WalletAbiFinalizerSpec::wallet();
        let input = WalletAbiInputSchema::from_sequence(
            "wallet-input",
            utxo_source.as_ref(),
            unblinding.as_ref(),
            sequence.as_ref(),
            finalizer.as_ref(),
        );
        let lock_variant =
            WalletAbiLockVariant::script(context.external_address.script_pubkey().as_ref());
        let asset_variant = WalletAbiAssetVariant::asset_id(policy_asset);
        let blinder_variant = WalletAbiBlinderVariant::explicit();
        let output = WalletAbiOutputSchema::new(
            "external",
            5_000,
            lock_variant.as_ref(),
            asset_variant.as_ref(),
            blinder_variant.as_ref(),
        );
        let params = WalletAbiRuntimeParams::new(&[input], &[output], Some(0.0), None);
        WalletAbiTxCreateRequest::from_parts(
            "0d6d53cd-a040-4f0c-8d28-c67b6608fb14",
            context.network.as_ref(),
            params.as_ref(),
            true,
        )
        .expect("request")
    }

    #[test]
    fn frozen_session_helpers_reuse_one_request_snapshot() {
        let context = test_context();
        let request = create_request(&context);
        let evaluate_request = WalletAbiTxEvaluateRequest::from_parts(
            &request.request_id(),
            request.network().as_ref(),
            request.params().as_ref(),
        )
        .expect("evaluate request");

        let session = context
            .provider
            .capture_request_session()
            .expect("capture session");
        let evaluate_response = context
            .provider
            .evaluate_request_with_session(&session, evaluate_request.as_ref())
            .expect("evaluate");
        let create_response = context
            .provider
            .process_request_with_session(&session, request.as_ref())
            .expect("process");

        assert_eq!(context.open_calls.load(Ordering::SeqCst), 1);
        assert!(context
            .output_session_ids
            .lock()
            .expect("session ids")
            .iter()
            .all(|session_id| session_id == "frozen-session"));
        assert_eq!(
            evaluate_response
                .preview()
                .expect("preview")
                .to_json()
                .expect("preview json"),
            create_response
                .preview()
                .expect("preview result")
                .expect("preview")
                .to_json()
                .expect("preview json")
        );
        assert_eq!(context.broadcast_calls.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn coordinator_roundtrip_ack_and_restore() {
        let context = test_context();
        let coordinator =
            WalletAbiWalletConnectCoordinator::new(context.provider.clone(), "wallet-1")
                .expect("coordinator");
        let proposal = WalletAbiWalletConnectSessionProposal {
            proposal_id: 7,
            pairing_uri: Some("wc:abc@2?symKey=01".to_owned()),
            name: "Requester".to_owned(),
            description: Some("Wallet ABI harness".to_owned()),
            url: Some("https://example.com".to_owned()),
            icons: Vec::new(),
            required_chain_ids: vec![WALLET_ABI_WALLETCONNECT_CHAIN_TESTNET.to_owned()],
            optional_chain_ids: Vec::new(),
            required_methods: vec![
                GET_SIGNER_RECEIVE_ADDRESS_METHOD.to_owned(),
                GET_RAW_SIGNING_X_ONLY_PUBKEY_METHOD.to_owned(),
                WALLET_ABI_PROCESS_REQUEST_METHOD.to_owned(),
            ],
            optional_methods: Vec::new(),
            required_events: Vec::new(),
            optional_events: Vec::new(),
        };

        assert!(coordinator
            .handle_session_proposal(proposal.clone())
            .expect("proposal")
            .is_empty());
        let approve_actions = coordinator
            .approve_current_overlay()
            .expect("approve overlay");
        assert_eq!(approve_actions.len(), 1);
        let approve_action = approve_actions[0].clone();
        coordinator
            .handle_approve_session_succeeded(
                &approve_action.action_id,
                WalletAbiWalletConnectSessionInfo {
                    topic: "topic-1".to_owned(),
                    chain_id: WALLET_ABI_WALLETCONNECT_CHAIN_TESTNET.to_owned(),
                    methods: proposal.required_methods.clone(),
                    accounts: vec!["walabi:testnet-liquid:wallet-1".to_owned()],
                    peer_name: Some("Requester".to_owned()),
                    peer_description: None,
                    peer_url: Some("https://example.com".to_owned()),
                    peer_icons: Vec::new(),
                },
            )
            .expect("approve ack");

        let request = create_request(&context);
        let request_json = request.to_json().expect("request json");
        assert!(coordinator
            .handle_session_request(WalletAbiWalletConnectSessionRequest {
                topic: "topic-1".to_owned(),
                request_id: 42,
                chain_id: WALLET_ABI_WALLETCONNECT_CHAIN_TESTNET.to_owned(),
                method: WALLET_ABI_PROCESS_REQUEST_METHOD.to_owned(),
                params_json: request_json.clone(),
            })
            .expect("request")
            .is_empty());
        let approve_request_actions = coordinator
            .approve_current_overlay()
            .expect("approve request");
        assert_eq!(approve_request_actions.len(), 1);
        coordinator
            .handle_transport_action_succeeded(&approve_request_actions[0].action_id)
            .expect("request ack");

        let snapshot = coordinator.snapshot_json().expect("snapshot");
        let restored = WalletAbiWalletConnectCoordinator::from_snapshot_json(
            context.provider,
            "wallet-1",
            &snapshot,
        )
        .expect("restore");
        let restored_state = restored.ui_state().expect("ui state");
        assert_eq!(restored_state.active_sessions.len(), 1);
        assert!(restored_state.current_overlay.is_none());

        let reconcile = restored
            .reconcile_pending_requests(vec![WalletAbiWalletConnectSessionRequest {
                topic: "topic-1".to_owned(),
                request_id: 42,
                chain_id: WALLET_ABI_WALLETCONNECT_CHAIN_TESTNET.to_owned(),
                method: WALLET_ABI_PROCESS_REQUEST_METHOD.to_owned(),
                params_json: request_json,
            }])
            .expect("reconcile");
        assert_eq!(reconcile.actions.len(), 1);
        assert!(reconcile.requests_to_replay.is_empty());
    }

    #[test]
    fn invalid_proposal_is_rejected_without_overlay() {
        let context = test_context();
        let coordinator = WalletAbiWalletConnectCoordinator::new(context.provider, "wallet-1")
            .expect("coordinator");
        let actions = coordinator
            .handle_session_proposal(WalletAbiWalletConnectSessionProposal {
                proposal_id: 9,
                pairing_uri: None,
                name: "Bad requester".to_owned(),
                description: None,
                url: None,
                icons: Vec::new(),
                required_chain_ids: vec!["walabi:liquid".to_owned()],
                optional_chain_ids: Vec::new(),
                required_methods: vec![GET_SIGNER_RECEIVE_ADDRESS_METHOD.to_owned()],
                optional_methods: Vec::new(),
                required_events: vec!["session_ping".to_owned()],
                optional_events: Vec::new(),
            })
            .expect("proposal");
        assert_eq!(actions.len(), 1);
        assert_eq!(
            actions[0].kind,
            WalletAbiWalletConnectTransportActionKind::RejectSession
        );
        assert!(coordinator
            .ui_state()
            .expect("ui state")
            .current_overlay
            .is_none());
    }
}
