use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use futures_util::{SinkExt, StreamExt};
use lwk_wollet::clients::blocking::BlockchainBackend;
use serde::Deserialize;
use tokio::net::TcpStream;
use tokio::runtime::{Builder, Runtime};
use tokio::time::timeout;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream};

use super::crypto::{decrypt_relay_payload, encrypt_relay_payload, WalletAbiRelayDirection};
use super::pairing::{parse_relay_pairing_input, WalletAbiRelayPairing};
use super::protocol::{
    WalletAbiRelayClientFrameV1, WalletAbiRelayFrameDirection, WalletAbiRelayRequestV2,
    WalletAbiRelayResponseError, WalletAbiRelayResponseV2, WalletAbiRelayRole,
    WalletAbiRelayServerFrameV1, WalletAbiRelayStatusState, WALLET_ABI_RELAY_VERSION,
};
use crate::{LwkError, Network, TxOut};

const RELAY_DEFAULT_WAIT_TIMEOUT_MS: u64 = 180_000;
const RELAY_RESPONSE_ACK_TIMEOUT_MS: u64 = 30_000;

static NEXT_RESPONSE_MSG_ID: AtomicU64 = AtomicU64::new(1);

type RelaySocket = WebSocketStream<MaybeTlsStream<TcpStream>>;

/// Fetch a transaction output from an esplora backend owned by the native layer.
///
/// `network` accepts `liquid`, `testnet-liquid`, or `localtest-liquid`.
/// `explorer_url`, when provided, is normalized to an API base URL before fetching.
#[uniffi::export]
pub fn web_connection_fetch_tx_out(
    txid: String,
    vout: u32,
    network: String,
    explorer_url: Option<String>,
) -> Result<Arc<TxOut>, LwkError> {
    let network = parse_wallet_abi_network(&network)?;
    let client = match normalize_explorer_api_base(explorer_url)? {
        Some(url) => crate::EsploraClient::new(&url, &network)?,
        None => network.default_esplora_client()?,
    };
    let txid = txid
        .parse::<elements::Txid>()
        .map_err(|error| LwkError::Generic {
            msg: format!("wallet-abi txid '{txid}' is invalid: {error}"),
        })?;
    let tx = client.inner.lock()?.get_transaction(txid)?;
    let output = tx
        .output
        .get(vout as usize)
        .cloned()
        .ok_or(LwkError::Generic {
            msg: format!(
                "wallet-abi tx output vout={vout} is missing (outputs={})",
                tx.output.len()
            ),
        })?;
    Ok(Arc::new(output.into()))
}

#[derive(uniffi::Record, Clone, Debug)]
pub struct WalletAbiRelayRequest {
    pub pairing_id: String,
    pub origin: String,
    pub request_id: String,
    pub created_at_ms: u64,
    pub method: String,
    pub request_json: String,
    pub network: Option<String>,
}

#[derive(uniffi::Enum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum WalletAbiRelayConnectionStatus {
    Connected,
    Closed,
    Expired,
    Error,
}

/// Wallet-ABI relay connection backed by a native websocket session.
#[derive(uniffi::Object)]
pub struct WalletAbiRelayConnection {
    runtime: Runtime,
    state: Mutex<WalletAbiRelayConnectionState>,
}

struct WalletAbiRelayConnectionState {
    session: Option<WalletAbiRelaySession>,
    connection_status: WalletAbiRelayConnectionStatus,
}

struct WalletAbiRelaySession {
    pairing: WalletAbiRelayPairing,
    socket: RelaySocket,
}

#[derive(Debug, Deserialize)]
struct WalletAbiJsonRpcRequestMeta {
    method: String,
    #[serde(default)]
    params: Option<serde_json::Value>,
}

enum RelayRequestReceiveError {
    Closed,
    Expired,
    Other(LwkError),
}

#[uniffi::export]
impl WalletAbiRelayConnection {
    /// Open a persistent relay websocket from a pairing payload.
    #[uniffi::constructor]
    pub fn open(input: String) -> Result<Arc<Self>, LwkError> {
        let runtime = Builder::new_multi_thread()
            .enable_all()
            .build()
            .map_err(|error| LwkError::Generic {
                msg: format!("wallet-abi relay runtime initialization failed: {error}"),
            })?;
        let session = runtime.block_on(open_session(input))?;

        Ok(Arc::new(Self {
            runtime,
            state: Mutex::new(WalletAbiRelayConnectionState {
                session: Some(session),
                connection_status: WalletAbiRelayConnectionStatus::Connected,
            }),
        }))
    }

    /// Wait for the next JSON-RPC request from the paired dApp.
    pub fn next_request(&self) -> Result<Option<WalletAbiRelayRequest>, LwkError> {
        let mut state = self.state.lock()?;

        let receive_result = {
            let session = match state.session.as_mut() {
                Some(session) => session,
                None => return Ok(None),
            };
            let timeout_ms =
                resolve_timeout_millis(&session.pairing, RELAY_DEFAULT_WAIT_TIMEOUT_MS)?;
            self.runtime.block_on(async {
                timeout(
                    Duration::from_millis(timeout_ms),
                    receive_next_request(session),
                )
                .await
            })
        };

        match receive_result {
            Ok(Ok(request)) => Ok(Some(request)),
            Ok(Err(RelayRequestReceiveError::Closed)) => {
                close_session_with_state(
                    &self.runtime,
                    &mut state,
                    WalletAbiRelayConnectionStatus::Closed,
                );
                Ok(None)
            }
            Ok(Err(RelayRequestReceiveError::Expired)) => {
                close_session_with_state(
                    &self.runtime,
                    &mut state,
                    WalletAbiRelayConnectionStatus::Expired,
                );
                Ok(None)
            }
            Ok(Err(RelayRequestReceiveError::Other(error))) => {
                close_session_with_state(
                    &self.runtime,
                    &mut state,
                    WalletAbiRelayConnectionStatus::Error,
                );
                Err(error)
            }
            Err(_) => Err(LwkError::Generic {
                msg: "wallet-abi relay request wait timed out".to_string(),
            }),
        }
    }

    /// Publish a JSON-RPC success/error envelope through the relay websocket.
    pub fn reply_success(
        &self,
        request: WalletAbiRelayRequest,
        response_json: String,
    ) -> Result<(), LwkError> {
        let json_rpc_response: serde_json::Value = serde_json::from_str(&response_json)?;
        let response_payload = WalletAbiRelayResponseV2 {
            v: WALLET_ABI_RELAY_VERSION,
            pairing_id: request.pairing_id,
            request_id: request.request_id,
            origin: request.origin,
            processed_at_ms: current_epoch_ms()?,
            json_rpc_response: Some(json_rpc_response),
            error: None,
        };

        self.with_session(|runtime, session| {
            runtime.block_on(publish_response(session, response_payload))
        })
    }

    /// Publish a relay-level error response through the websocket.
    pub fn reply_error(
        &self,
        request: WalletAbiRelayRequest,
        code: String,
        message: String,
    ) -> Result<(), LwkError> {
        let response_payload = WalletAbiRelayResponseV2 {
            v: WALLET_ABI_RELAY_VERSION,
            pairing_id: request.pairing_id,
            request_id: request.request_id,
            origin: request.origin,
            processed_at_ms: current_epoch_ms()?,
            json_rpc_response: None,
            error: Some(WalletAbiRelayResponseError { code, message }),
        };

        self.with_session(|runtime, session| {
            runtime.block_on(publish_response(session, response_payload))
        })
    }

    /// Return the current session state.
    pub fn connection_state(&self) -> WalletAbiRelayConnectionStatus {
        self.state
            .lock()
            .map(|state| state.connection_status)
            .unwrap_or(WalletAbiRelayConnectionStatus::Error)
    }
}

impl WalletAbiRelayConnection {
    fn with_session<T>(
        &self,
        operation: impl FnOnce(&Runtime, &mut WalletAbiRelaySession) -> Result<T, LwkError>,
    ) -> Result<T, LwkError> {
        let mut state = self.state.lock()?;
        let session = state.session.as_mut().ok_or(LwkError::ObjectConsumed)?;
        operation(&self.runtime, session)
    }

    fn close_inner(&self, connection_status: WalletAbiRelayConnectionStatus) {
        if let Ok(mut state) = self.state.lock() {
            close_session_with_state(&self.runtime, &mut state, connection_status);
        }
    }
}

impl Drop for WalletAbiRelayConnection {
    fn drop(&mut self) {
        self.close_inner(WalletAbiRelayConnectionStatus::Closed);
    }
}

fn close_session_with_state(
    runtime: &Runtime,
    state: &mut WalletAbiRelayConnectionState,
    connection_status: WalletAbiRelayConnectionStatus,
) {
    state.connection_status = connection_status;
    if let Some(session) = state.session.take() {
        let _ = runtime.block_on(close_quietly(session));
    }
}

fn parse_wallet_abi_network(network: &str) -> Result<Arc<Network>, LwkError> {
    match network.trim() {
        "liquid" => Ok(Network::mainnet()),
        "testnet-liquid" => Ok(Network::testnet()),
        "localtest-liquid" => Ok(Network::regtest_default()),
        other => Err(LwkError::Generic {
            msg: format!("wallet-abi network '{other}' is unsupported"),
        }),
    }
}

fn normalize_explorer_api_base(explorer_url: Option<String>) -> Result<Option<String>, LwkError> {
    let Some(explorer_url) = explorer_url else {
        return Ok(None);
    };
    let trimmed = explorer_url.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }

    let normalized = trimmed
        .split("/tx/")
        .next()
        .unwrap_or(trimmed)
        .split("/tx")
        .next()
        .unwrap_or(trimmed)
        .trim_end_matches('/');
    if normalized.is_empty() {
        return Err(LwkError::Generic {
            msg: "wallet-abi explorer url is invalid".to_string(),
        });
    }

    Ok(Some(if normalized.ends_with("/api") {
        normalized.to_string()
    } else {
        format!("{normalized}/api")
    }))
}

async fn open_session(input: String) -> Result<WalletAbiRelaySession, LwkError> {
    let pairing = parse_relay_pairing_input(&input)?;
    let (socket, _) = connect_async(&pairing.relay_ws_url)
        .await
        .map_err(|error| LwkError::Generic {
            msg: format!("wallet-abi relay websocket connection failed: {error}"),
        })?;
    let mut session = WalletAbiRelaySession { pairing, socket };

    match send_frame(
        &mut session.socket,
        &WalletAbiRelayClientFrameV1::Auth {
            pairing_id: session.pairing.pairing_id.clone(),
            role: WalletAbiRelayRole::Phone,
            token: session.pairing.phone_token.clone(),
        },
    )
    .await
    {
        Ok(()) => Ok(session),
        Err(error) => {
            close_quietly(session).await;
            Err(error)
        }
    }
}

async fn receive_next_request(
    session: &mut WalletAbiRelaySession,
) -> Result<WalletAbiRelayRequest, RelayRequestReceiveError> {
    loop {
        match receive_server_frame(&mut session.socket).await {
            Ok(WalletAbiRelayServerFrameV1::Error { code, message, .. }) => {
                return Err(RelayRequestReceiveError::Other(LwkError::Generic {
                    msg: format!("wallet-abi relay error '{code}': {message}"),
                }));
            }
            Ok(WalletAbiRelayServerFrameV1::Status { state, .. }) => {
                if state == WalletAbiRelayStatusState::Expired {
                    return Err(RelayRequestReceiveError::Expired);
                }
            }
            Ok(WalletAbiRelayServerFrameV1::Deliver {
                pairing_id,
                direction,
                msg_id,
                nonce_b64,
                ciphertext_b64,
                created_at_ms,
                ..
            }) => {
                if pairing_id != session.pairing.pairing_id
                    || direction != WalletAbiRelayFrameDirection::WebToPhone
                {
                    continue;
                }

                let plaintext = decrypt_relay_payload(
                    &session.pairing.channel_key_b64,
                    &session.pairing.pairing_id,
                    WalletAbiRelayDirection::WebToPhone,
                    &msg_id,
                    &nonce_b64,
                    &ciphertext_b64,
                )
                .map_err(RelayRequestReceiveError::Other)?;
                let relay_request = parse_relay_request_payload(&plaintext)
                    .map_err(RelayRequestReceiveError::Other)?;
                let request_json = serde_json::to_string(&relay_request.json_rpc_request)
                    .map_err(Into::<LwkError>::into)
                    .map_err(RelayRequestReceiveError::Other)?;
                let (method, network) = parse_json_rpc_request_metadata(&request_json)
                    .map_err(RelayRequestReceiveError::Other)?;

                send_frame(
                    &mut session.socket,
                    &WalletAbiRelayClientFrameV1::Ack {
                        pairing_id: session.pairing.pairing_id.clone(),
                        msg_id,
                    },
                )
                .await
                .map_err(RelayRequestReceiveError::Other)?;

                return Ok(WalletAbiRelayRequest {
                    pairing_id: relay_request.pairing_id,
                    origin: relay_request.origin,
                    request_id: relay_request.request_id,
                    created_at_ms,
                    method,
                    request_json,
                    network,
                });
            }
            Ok(WalletAbiRelayServerFrameV1::Ack { .. }) => {}
            Err(error) => {
                if is_closed_error(&error) {
                    return Err(RelayRequestReceiveError::Closed);
                }
                return Err(RelayRequestReceiveError::Other(error));
            }
        }
    }
}

async fn publish_response(
    session: &mut WalletAbiRelaySession,
    response_payload: WalletAbiRelayResponseV2,
) -> Result<(), LwkError> {
    let response_bytes = serde_json::to_vec(&response_payload)?;
    let response_msg_id = next_response_msg_id();
    let encrypted = encrypt_relay_payload(
        &session.pairing.channel_key_b64,
        &session.pairing.pairing_id,
        WalletAbiRelayDirection::PhoneToWeb,
        &response_msg_id,
        &response_bytes,
    )?;

    send_frame(
        &mut session.socket,
        &WalletAbiRelayClientFrameV1::Publish {
            pairing_id: session.pairing.pairing_id.clone(),
            direction: WalletAbiRelayFrameDirection::PhoneToWeb,
            msg_id: response_msg_id.clone(),
            nonce_b64: encrypted.nonce_b64,
            ciphertext_b64: encrypted.ciphertext_b64,
        },
    )
    .await?;

    timeout(
        Duration::from_millis(RELAY_RESPONSE_ACK_TIMEOUT_MS),
        async {
            loop {
                match receive_server_frame(&mut session.socket).await? {
                    WalletAbiRelayServerFrameV1::Ack { pairing_id, msg_id } => {
                        if pairing_id == session.pairing.pairing_id && msg_id == response_msg_id {
                            return Ok(());
                        }
                    }
                    WalletAbiRelayServerFrameV1::Error { code, message, .. } => {
                        return Err(LwkError::Generic {
                            msg: format!("wallet-abi relay error '{code}': {message}"),
                        });
                    }
                    WalletAbiRelayServerFrameV1::Status { state, detail, .. } => {
                        if state == WalletAbiRelayStatusState::Expired {
                            return Err(LwkError::Generic {
                                msg: format!("wallet-abi relay pairing expired: {detail}"),
                            });
                        }
                    }
                    WalletAbiRelayServerFrameV1::Deliver {
                        pairing_id,
                        direction,
                        msg_id,
                        ..
                    } => {
                        if pairing_id == session.pairing.pairing_id
                            && direction == WalletAbiRelayFrameDirection::WebToPhone
                        {
                            send_frame(
                                &mut session.socket,
                                &WalletAbiRelayClientFrameV1::Ack { pairing_id, msg_id },
                            )
                            .await?;
                        }
                    }
                }
            }
        },
    )
    .await
    .map_err(|_| LwkError::Generic {
        msg: "wallet-abi relay response acknowledgement timed out".to_string(),
    })?
}

async fn close_quietly(mut session: WalletAbiRelaySession) {
    let _ = session.socket.close(None).await;
}

fn parse_relay_request_payload(plaintext: &[u8]) -> Result<WalletAbiRelayRequestV2, LwkError> {
    let relay_request: WalletAbiRelayRequestV2 = serde_json::from_slice(plaintext)?;
    if relay_request.v != WALLET_ABI_RELAY_VERSION {
        return Err(LwkError::Generic {
            msg: format!(
                "wallet-abi relay request version '{}' is unsupported",
                relay_request.v
            ),
        });
    }
    Ok(relay_request)
}

fn parse_json_rpc_request_metadata(
    request_json: &str,
) -> Result<(String, Option<String>), LwkError> {
    let meta: WalletAbiJsonRpcRequestMeta = serde_json::from_str(request_json)?;
    let network = meta
        .params
        .as_ref()
        .and_then(|params| params.as_object())
        .and_then(|params| params.get("network"))
        .and_then(serde_json::Value::as_str)
        .map(ToOwned::to_owned);
    Ok((meta.method, network))
}

async fn receive_server_frame(
    socket: &mut RelaySocket,
) -> Result<WalletAbiRelayServerFrameV1, LwkError> {
    loop {
        let Some(message_result) = socket.next().await else {
            return Err(LwkError::Generic {
                msg: "wallet-abi relay websocket closed".to_string(),
            });
        };
        let message = message_result.map_err(|error| LwkError::Generic {
            msg: format!("wallet-abi relay websocket read failed: {error}"),
        })?;
        match message {
            Message::Text(text) => {
                return serde_json::from_str(text.as_ref()).map_err(Into::into);
            }
            Message::Close(_) => {
                return Err(LwkError::Generic {
                    msg: "wallet-abi relay websocket closed".to_string(),
                });
            }
            Message::Binary(_) | Message::Ping(_) | Message::Pong(_) | Message::Frame(_) => {}
        }
    }
}

async fn send_frame(
    socket: &mut RelaySocket,
    frame: &WalletAbiRelayClientFrameV1,
) -> Result<(), LwkError> {
    let payload = serde_json::to_string(frame)?;
    socket
        .send(Message::Text(payload.into()))
        .await
        .map_err(|error| LwkError::Generic {
            msg: format!("wallet-abi relay websocket write failed: {error}"),
        })
}

fn resolve_timeout_millis(
    pairing: &WalletAbiRelayPairing,
    requested_timeout_ms: u64,
) -> Result<u64, LwkError> {
    let now_ms = current_epoch_ms()?;
    let remaining_ms = pairing.expires_at_ms.saturating_sub(now_ms);
    if remaining_ms == 0 {
        return Err(LwkError::Generic {
            msg: "wallet-abi relay pairing has expired".to_string(),
        });
    }

    Ok(remaining_ms.min(requested_timeout_ms.max(1)))
}

fn is_closed_error(error: &LwkError) -> bool {
    matches!(
        error,
        LwkError::Generic { msg } if msg.contains("wallet-abi relay websocket closed")
    )
}

pub(crate) fn current_epoch_ms() -> Result<u64, LwkError> {
    let elapsed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| LwkError::Generic {
            msg: format!("wallet-abi relay time error: {error}"),
        })?;
    Ok(elapsed.as_millis() as u64)
}

fn next_response_msg_id() -> String {
    format!(
        "phone-response-{}",
        NEXT_RESPONSE_MSG_ID.fetch_add(1, Ordering::Relaxed)
    )
}
