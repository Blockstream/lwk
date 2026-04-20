use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::sync::{Arc, Mutex};

use crate::network::Network;
use crate::{LwkError, WalletAbiProvider};

use lwk_simplicity::wallet_abi::schema as abi;
use lwk_simplicity::wallet_abi::{
    GET_RAW_SIGNING_X_ONLY_PUBKEY_METHOD, GET_SIGNER_RECEIVE_ADDRESS_METHOD,
    WALLET_ABI_PROCESS_REQUEST_METHOD,
};

const WALLET_ABI_WALLETCONNECT_CHAIN_MAINNET: &str = "walabi:liquid";
const WALLET_ABI_WALLETCONNECT_CHAIN_TESTNET: &str = "walabi:testnet-liquid";
const WALLET_ABI_WALLETCONNECT_CHAIN_REGTEST: &str = "walabi:localtest-liquid";

const USER_REJECTED_MESSAGE: &str = "wallet connect request rejected by user";

const SUPPORTED_METHODS: &[&str] = &[
    GET_SIGNER_RECEIVE_ADDRESS_METHOD,
    GET_RAW_SIGNING_X_ONLY_PUBKEY_METHOD,
    WALLET_ABI_PROCESS_REQUEST_METHOD,
];

/// Thin WalletConnect-facing coordinator that owns wallet-side session approval state.
#[derive(uniffi::Object)]
pub struct WalletAbiWalletConnectCoordinator {
    wallet_id: String,
    provider_chain_id: String,
    state: Mutex<CoordinatorState>,
}

/// WalletConnect session proposal surfaced to the coordinator by the app.
#[allow(missing_docs)]
#[derive(uniffi::Record, Clone, Debug, PartialEq, Eq)]
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
#[derive(uniffi::Record, Clone, Debug, PartialEq, Eq)]
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

/// UI overlay kind emitted by the coordinator.
#[allow(missing_docs)]
#[derive(uniffi::Enum, Clone, Copy, Debug, PartialEq, Eq)]
pub enum WalletAbiWalletConnectOverlayKind {
    ConnectionApproval,
}

/// Current UI overlay the app should render.
#[allow(missing_docs)]
#[derive(uniffi::Record, Clone, Debug, PartialEq, Eq)]
pub struct WalletAbiWalletConnectOverlay {
    pub kind: WalletAbiWalletConnectOverlayKind,
    pub chain_id: String,
    pub proposal: WalletAbiWalletConnectSessionProposal,
    pub awaiting_transport: bool,
}

/// UI state snapshot for the app.
#[allow(missing_docs)]
#[derive(uniffi::Record, Clone, Debug, PartialEq, Eq)]
pub struct WalletAbiWalletConnectUiState {
    pub active_sessions: Vec<WalletAbiWalletConnectSessionInfo>,
    pub current_overlay: Option<WalletAbiWalletConnectOverlay>,
    pub queued_overlay_count: u32,
    pub last_error: Option<String>,
    pub pending_action_count: u32,
}

/// Semantic WalletConnect reason kind for session reject/disconnect actions.
#[allow(missing_docs)]
#[derive(uniffi::Enum, Clone, Copy, Debug, PartialEq, Eq)]
pub enum WalletAbiWalletConnectReasonKind {
    UserRejected,
    UserDisconnected,
    UnsupportedProposal,
    ReplacedSession,
    SessionDeleted,
}

/// Semantic transport action kind returned by the coordinator.
#[allow(missing_docs)]
#[derive(uniffi::Enum, Clone, Copy, Debug, PartialEq, Eq)]
pub enum WalletAbiWalletConnectTransportActionKind {
    ApproveSession,
    RejectSession,
    DisconnectSession,
}

/// Approve-session action payload.
#[allow(missing_docs)]
#[derive(uniffi::Record, Clone, Debug, PartialEq, Eq)]
pub struct WalletAbiWalletConnectApproveSessionAction {
    pub proposal_id: u64,
    pub chain_id: String,
    pub methods: Vec<String>,
    pub accounts: Vec<String>,
}

/// Reject-session action payload.
#[allow(missing_docs)]
#[derive(uniffi::Record, Clone, Debug, PartialEq, Eq)]
pub struct WalletAbiWalletConnectRejectSessionAction {
    pub proposal_id: u64,
    pub chain_id: String,
    pub reason_kind: WalletAbiWalletConnectReasonKind,
    pub message: String,
}

/// Disconnect-session action payload.
#[allow(missing_docs)]
#[derive(uniffi::Record, Clone, Debug, PartialEq, Eq)]
pub struct WalletAbiWalletConnectDisconnectSessionAction {
    pub topic: String,
    pub chain_id: String,
    pub reason_kind: WalletAbiWalletConnectReasonKind,
    pub message: String,
}

/// One semantic transport action the app must execute.
#[allow(missing_docs)]
#[derive(uniffi::Record, Clone, Debug, PartialEq, Eq)]
pub struct WalletAbiWalletConnectTransportAction {
    pub action_id: String,
    pub kind: WalletAbiWalletConnectTransportActionKind,
    pub approve_session: Option<WalletAbiWalletConnectApproveSessionAction>,
    pub reject_session: Option<WalletAbiWalletConnectRejectSessionAction>,
    pub disconnect_session: Option<WalletAbiWalletConnectDisconnectSessionAction>,
}

#[derive(Default)]
struct CoordinatorState {
    active_sessions: BTreeMap<String, WalletAbiWalletConnectSessionInfo>,
    overlays: VecDeque<ConnectionOverlayState>,
    pending_actions: BTreeMap<String, PendingActionEntry>,
    last_error: Option<String>,
}

#[derive(Clone, Debug)]
struct ConnectionOverlayState {
    proposal: WalletAbiWalletConnectSessionProposal,
    awaiting_transport: bool,
    decision: Option<ConnectionOverlayDecision>,
    pending_action_ids: Vec<String>,
}

#[derive(Clone, Copy, Debug)]
enum ConnectionOverlayDecision {
    Approve,
    Reject,
}

#[derive(Clone, Debug)]
struct PendingActionEntry {
    action: WalletAbiWalletConnectTransportAction,
    state: PendingActionState,
}

#[derive(Clone, Debug)]
enum PendingActionState {
    ApproveSession {
        chain_id: String,
        expected_methods: Vec<String>,
        expected_accounts: Vec<String>,
    },
    RejectSession,
    DisconnectSession {
        chain_id: String,
        topic: String,
    },
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
        let provider_chain_id =
            network_to_wallet_connect_chain(provider.get_capabilities()?.network().as_ref())?;

        Ok(Self {
            wallet_id: wallet_id.to_owned(),
            provider_chain_id,
            state: Mutex::new(CoordinatorState::default()),
        })
    }

    /// Return the current UI-facing state snapshot.
    pub fn ui_state(&self) -> Result<WalletAbiWalletConnectUiState, LwkError> {
        let state = self.state.lock()?;
        Ok(WalletAbiWalletConnectUiState {
            active_sessions: state.active_sessions.values().cloned().collect(),
            current_overlay: state.overlays.front().map(overlay_to_public),
            queued_overlay_count: state.overlays.len().saturating_sub(1) as u32,
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
            .any(|overlay| overlay.proposal.proposal_id == proposal.proposal_id)
        {
            return Ok(Vec::new());
        }

        match self.validate_proposal(&proposal) {
            Ok(_) => {
                state.overlays.push_back(ConnectionOverlayState {
                    proposal,
                    awaiting_transport: false,
                    decision: None,
                    pending_action_ids: Vec::new(),
                });
                state.last_error = None;
                Ok(Vec::new())
            }
            Err(message) => {
                let chain_id = first_requested_chain_id(&proposal)
                    .unwrap_or_else(|| self.provider_chain_id.clone());
                let action = reject_session_action(
                    proposal.proposal_id,
                    &chain_id,
                    WalletAbiWalletConnectReasonKind::UnsupportedProposal,
                    &message.to_string(),
                );
                insert_pending_action(
                    &mut state,
                    action.clone(),
                    PendingActionState::RejectSession,
                );
                state.last_error = Some(message.to_string());
                Ok(vec![action])
            }
        }
    }

    /// Handle one peer-driven session delete.
    pub fn handle_session_delete(&self, topic: &str) -> Result<(), LwkError> {
        let mut state = self.state.lock()?;
        state
            .active_sessions
            .retain(|_, session| session.topic != topic);
        state.last_error = None;
        Ok(())
    }

    /// Handle one peer-driven session update/extend.
    pub fn handle_session_extend(
        &self,
        session_info: WalletAbiWalletConnectSessionInfo,
    ) -> Result<(), LwkError> {
        if session_info.chain_id != self.provider_chain_id {
            return Err(LwkError::from(
                "session chain does not match coordinator provider chain",
            ));
        }
        let mut state = self.state.lock()?;
        state
            .active_sessions
            .insert(session_info.chain_id.clone(), session_info);
        Ok(())
    }

    /// Approve the current connection overlay.
    pub fn approve_current_overlay(
        &self,
    ) -> Result<Vec<WalletAbiWalletConnectTransportAction>, LwkError> {
        let mut state = self.state.lock()?;
        let Some(current) = state.overlays.front().cloned() else {
            return Err(LwkError::from("no wallet connect overlay to approve"));
        };

        if !current.pending_action_ids.is_empty() {
            return pending_actions_by_ids(&state, &current.pending_action_ids);
        }

        let validated = self.validate_proposal(&current.proposal)?;
        let mut actions = Vec::new();

        if let Some(existing) = state.active_sessions.get(&validated.chain_id).cloned() {
            let disconnect_action = disconnect_session_action(
                &existing.topic,
                &existing.chain_id,
                WalletAbiWalletConnectReasonKind::ReplacedSession,
                "wallet connect session replaced by a new approval",
            );
            insert_pending_action(
                &mut state,
                disconnect_action.clone(),
                PendingActionState::DisconnectSession {
                    chain_id: existing.chain_id,
                    topic: existing.topic,
                },
            );
            actions.push(disconnect_action);
        }

        let approve_action = approve_session_action(
            current.proposal.proposal_id,
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
                    .expect("approve payload")
                    .methods
                    .clone(),
                expected_accounts: approve_action
                    .approve_session
                    .as_ref()
                    .expect("approve payload")
                    .accounts
                    .clone(),
            },
        );
        actions.push(approve_action);

        let Some(front) = state.overlays.front_mut() else {
            return Err(LwkError::from(
                "wallet connect overlay changed while approving",
            ));
        };
        front.awaiting_transport = true;
        front.decision = Some(ConnectionOverlayDecision::Approve);
        front.pending_action_ids = actions
            .iter()
            .map(|action| action.action_id.clone())
            .collect();
        Ok(actions)
    }

    /// Reject the current connection overlay.
    pub fn reject_current_overlay(
        &self,
    ) -> Result<Vec<WalletAbiWalletConnectTransportAction>, LwkError> {
        let mut state = self.state.lock()?;
        let Some(current) = state.overlays.front().cloned() else {
            return Err(LwkError::from("no wallet connect overlay to reject"));
        };

        if !current.pending_action_ids.is_empty() {
            return Err(LwkError::from(
                "wallet connect overlay is awaiting transport acknowledgement",
            ));
        }

        let chain_id = self
            .validate_proposal(&current.proposal)
            .map(|proposal| proposal.chain_id)
            .unwrap_or_else(|_| {
                first_requested_chain_id(&current.proposal)
                    .unwrap_or_else(|| self.provider_chain_id.clone())
            });
        let action = reject_session_action(
            current.proposal.proposal_id,
            &chain_id,
            WalletAbiWalletConnectReasonKind::UserRejected,
            USER_REJECTED_MESSAGE,
        );
        insert_pending_action(
            &mut state,
            action.clone(),
            PendingActionState::RejectSession,
        );
        let Some(front) = state.overlays.front_mut() else {
            return Err(LwkError::from(
                "wallet connect overlay changed while rejecting",
            ));
        };
        front.awaiting_transport = true;
        front.decision = Some(ConnectionOverlayDecision::Reject);
        front.pending_action_ids = vec![action.action_id.clone()];
        Ok(vec![action])
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
            .insert(chain_id, confirmed_session_info);
        finish_overlay_action(&mut state, action_id, true);
        Ok(())
    }

    /// Ack one successful reject/disconnect transport action.
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

    /// Clear the last surfaced coordinator error.
    pub fn clear_error(&self) -> Result<(), LwkError> {
        let mut state = self.state.lock()?;
        state.last_error = None;
        Ok(())
    }
}

impl WalletAbiWalletConnectCoordinator {
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
}

fn overlay_to_public(overlay: &ConnectionOverlayState) -> WalletAbiWalletConnectOverlay {
    WalletAbiWalletConnectOverlay {
        kind: WalletAbiWalletConnectOverlayKind::ConnectionApproval,
        chain_id: first_requested_chain_id(&overlay.proposal).unwrap_or_default(),
        proposal: overlay.proposal.clone(),
        awaiting_transport: overlay.awaiting_transport,
    }
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
        if remove_pending_action_id(&mut front.pending_action_ids, action_id) {
            front.awaiting_transport = !front.pending_action_ids.is_empty();
            if success && front.pending_action_ids.is_empty() && front.decision.is_some() {
                should_pop_front = true;
            }
        }
    }
    if should_pop_front {
        state.overlays.pop_front();
    }
}

fn fail_overlay_action(state: &mut CoordinatorState, action_id: &str) {
    if let Some(front) = state.overlays.front_mut() {
        if remove_pending_action_id(&mut front.pending_action_ids, action_id) {
            front.awaiting_transport = !front.pending_action_ids.is_empty();
        }
    }
}

fn remove_pending_action_id(ids: &mut Vec<String>, action_id: &str) -> bool {
    let before = ids.len();
    ids.retain(|id| id != action_id);
    before != ids.len()
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
        disconnect_session: Some(WalletAbiWalletConnectDisconnectSessionAction {
            topic: topic.to_owned(),
            chain_id: chain_id.to_owned(),
            reason_kind,
            message: message.to_owned(),
        }),
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use crate::{
        Address, LwkError, Mnemonic, Network, Signer, SignerMetaLink, Transaction, TxOut,
        TxOutSecrets, Txid, WalletAbiBroadcasterCallbacks, WalletAbiOutputAllocatorCallbacks,
        WalletAbiPrevoutResolverCallbacks, WalletAbiReceiveAddressProviderCallbacks,
        WalletAbiRequestSession, WalletAbiSessionFactoryCallbacks, WalletAbiSignerContext,
        WalletAbiWalletOutputRequest, WalletAbiWalletOutputTemplate, WalletBroadcasterLink,
        WalletOutputAllocatorLink, WalletPrevoutResolverLink, WalletReceiveAddressProviderLink,
        WalletRuntimeDepsLink, WalletSessionFactoryLink,
    };

    struct TestSessionFactoryCallbacks;

    impl WalletAbiSessionFactoryCallbacks for TestSessionFactoryCallbacks {
        fn open_wallet_request_session(&self) -> Result<WalletAbiRequestSession, LwkError> {
            Ok(WalletAbiRequestSession {
                session_id: "capabilities-session".to_owned(),
                network: Network::testnet(),
                spendable_utxos: Vec::new(),
            })
        }
    }

    struct TestOutputAllocatorCallbacks;

    impl WalletAbiOutputAllocatorCallbacks for TestOutputAllocatorCallbacks {
        fn get_wallet_output_template(
            &self,
            _session: WalletAbiRequestSession,
            _request: WalletAbiWalletOutputRequest,
        ) -> Result<WalletAbiWalletOutputTemplate, LwkError> {
            unreachable!("not used in coordinator approval tests")
        }
    }

    struct TestPrevoutResolverCallbacks;

    impl WalletAbiPrevoutResolverCallbacks for TestPrevoutResolverCallbacks {
        fn get_bip32_derivation_pair(
            &self,
            _outpoint: Arc<crate::OutPoint>,
        ) -> Result<Option<crate::WalletAbiBip32DerivationPair>, LwkError> {
            unreachable!("not used in coordinator approval tests")
        }

        fn unblind(&self, _tx_out: Arc<TxOut>) -> Result<Arc<TxOutSecrets>, LwkError> {
            unreachable!("not used in coordinator approval tests")
        }

        fn get_tx_out(&self, _outpoint: Arc<crate::OutPoint>) -> Result<Arc<TxOut>, LwkError> {
            unreachable!("not used in coordinator approval tests")
        }
    }

    struct TestBroadcasterCallbacks;

    impl WalletAbiBroadcasterCallbacks for TestBroadcasterCallbacks {
        fn broadcast_transaction(&self, _tx: Arc<Transaction>) -> Result<Arc<Txid>, LwkError> {
            unreachable!("not used in coordinator approval tests")
        }
    }

    struct TestReceiveAddressProviderCallbacks;

    impl WalletAbiReceiveAddressProviderCallbacks for TestReceiveAddressProviderCallbacks {
        fn get_signer_receive_address(&self) -> Result<Arc<Address>, LwkError> {
            Address::new(
                "tlq1qq2xvpcvfup5j8zscjq05u2wxxjcyewk7979f3mmz5l7uw5pqmx6xf5xy50hsn6vhkm5euwt72x878eq6zxx2z58hd7zrsg9qn",
            )
        }
    }

    fn test_provider() -> Arc<WalletAbiProvider> {
        let network = Network::testnet();
        let mnemonic = Mnemonic::new(lwk_test_util::TEST_MNEMONIC).expect("mnemonic");
        let signer = Signer::new(&mnemonic, &network).expect("signer");
        let signer_link = SignerMetaLink::from_software_signer(
            signer,
            WalletAbiSignerContext {
                network: network.clone(),
                account_index: 0,
            },
        )
        .expect("signer link");

        Arc::new(WalletAbiProvider::new(
            Arc::new(signer_link),
            Arc::new(WalletRuntimeDepsLink::new(
                Arc::new(WalletSessionFactoryLink::new(Arc::new(
                    TestSessionFactoryCallbacks,
                ))),
                Arc::new(WalletOutputAllocatorLink::new(Arc::new(
                    TestOutputAllocatorCallbacks,
                ))),
                Arc::new(WalletPrevoutResolverLink::new(Arc::new(
                    TestPrevoutResolverCallbacks,
                ))),
                Arc::new(WalletBroadcasterLink::new(Arc::new(
                    TestBroadcasterCallbacks,
                ))),
                Arc::new(WalletReceiveAddressProviderLink::new(Arc::new(
                    TestReceiveAddressProviderCallbacks,
                ))),
            )),
        ))
    }

    fn valid_proposal(proposal_id: u64) -> WalletAbiWalletConnectSessionProposal {
        WalletAbiWalletConnectSessionProposal {
            proposal_id,
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
        }
    }

    #[test]
    fn session_approval_ack_creates_active_session() {
        let coordinator = WalletAbiWalletConnectCoordinator::new(test_provider(), "wallet-1")
            .expect("coordinator");
        let proposal = valid_proposal(7);

        assert!(coordinator
            .handle_session_proposal(proposal.clone())
            .expect("proposal")
            .is_empty());
        let overlay = coordinator
            .ui_state()
            .expect("ui state")
            .current_overlay
            .expect("overlay");
        assert_eq!(
            overlay.kind,
            WalletAbiWalletConnectOverlayKind::ConnectionApproval
        );

        let actions = coordinator
            .approve_current_overlay()
            .expect("approve overlay");
        assert_eq!(actions.len(), 1);
        assert_eq!(
            actions[0].kind,
            WalletAbiWalletConnectTransportActionKind::ApproveSession
        );
        assert_eq!(
            actions[0]
                .approve_session
                .as_ref()
                .expect("approve session")
                .accounts,
            vec!["walabi:testnet-liquid:wallet-1".to_owned()]
        );

        coordinator
            .handle_approve_session_succeeded(
                &actions[0].action_id,
                WalletAbiWalletConnectSessionInfo {
                    topic: "topic-1".to_owned(),
                    chain_id: WALLET_ABI_WALLETCONNECT_CHAIN_TESTNET.to_owned(),
                    methods: proposal.required_methods.clone(),
                    accounts: vec!["walabi:testnet-liquid:wallet-1".to_owned()],
                    peer_name: Some("Requester".to_owned()),
                    peer_description: Some("Wallet ABI harness".to_owned()),
                    peer_url: Some("https://example.com".to_owned()),
                    peer_icons: Vec::new(),
                },
            )
            .expect("approve ack");

        let state = coordinator.ui_state().expect("ui state");
        assert_eq!(state.active_sessions.len(), 1);
        assert!(state.current_overlay.is_none());
    }

    #[test]
    fn approving_new_session_disconnects_old_session_first() {
        let coordinator = WalletAbiWalletConnectCoordinator::new(test_provider(), "wallet-1")
            .expect("coordinator");
        let first = valid_proposal(7);
        let second = valid_proposal(8);

        coordinator
            .handle_session_proposal(first.clone())
            .expect("first proposal");
        let first_actions = coordinator
            .approve_current_overlay()
            .expect("approve first");
        coordinator
            .handle_approve_session_succeeded(
                &first_actions[0].action_id,
                WalletAbiWalletConnectSessionInfo {
                    topic: "topic-1".to_owned(),
                    chain_id: WALLET_ABI_WALLETCONNECT_CHAIN_TESTNET.to_owned(),
                    methods: first.required_methods.clone(),
                    accounts: vec!["walabi:testnet-liquid:wallet-1".to_owned()],
                    peer_name: Some("Requester".to_owned()),
                    peer_description: None,
                    peer_url: None,
                    peer_icons: Vec::new(),
                },
            )
            .expect("first ack");

        coordinator
            .handle_session_proposal(second.clone())
            .expect("second proposal");
        let second_actions = coordinator
            .approve_current_overlay()
            .expect("approve second");

        assert_eq!(second_actions.len(), 2);
        assert_eq!(
            second_actions[0].kind,
            WalletAbiWalletConnectTransportActionKind::DisconnectSession
        );
        assert_eq!(
            second_actions[1].kind,
            WalletAbiWalletConnectTransportActionKind::ApproveSession
        );

        coordinator
            .handle_transport_action_succeeded(&second_actions[0].action_id)
            .expect("disconnect ack");
        coordinator
            .handle_approve_session_succeeded(
                &second_actions[1].action_id,
                WalletAbiWalletConnectSessionInfo {
                    topic: "topic-2".to_owned(),
                    chain_id: WALLET_ABI_WALLETCONNECT_CHAIN_TESTNET.to_owned(),
                    methods: second.required_methods.clone(),
                    accounts: vec!["walabi:testnet-liquid:wallet-1".to_owned()],
                    peer_name: Some("Requester".to_owned()),
                    peer_description: None,
                    peer_url: None,
                    peer_icons: Vec::new(),
                },
            )
            .expect("second ack");

        let state = coordinator.ui_state().expect("ui state");
        assert_eq!(state.active_sessions.len(), 1);
        assert_eq!(state.active_sessions[0].topic, "topic-2");
    }

    #[test]
    fn invalid_proposal_is_rejected_without_overlay() {
        let coordinator = WalletAbiWalletConnectCoordinator::new(test_provider(), "wallet-1")
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
        let state = coordinator.ui_state().expect("ui state");
        assert!(state.current_overlay.is_none());
        assert_eq!(state.pending_action_count, 1);
    }
}
