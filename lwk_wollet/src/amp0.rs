use lwk_common::{Network, Stream};
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::Error;

pub struct Amp0<S: Stream> {
    stream: S,
}

impl<S: Stream> Amp0<S> {
    pub async fn new(stream: S) -> Result<Self, Error> {
        Ok(Self { stream })
    }
}

impl Amp0<WebSocketClient> {
    pub async fn with_network(network: Network) -> Result<Self, Error> {
        let url = match network {
            Network::Liquid => "wss://green-liquid-mainnet.blockstream.com/v2/ws/",
            Network::TestnetLiquid => "wss://green-liquid-testnet.blockstream.com/v2/ws/",
            Network::LocaltestLiquid => {
                return Err(Error::Generic(
                    "LocaltestLiquid is not supported".to_string(),
                ));
            }
        };

        Ok(Self {
            stream: WebSocketClient::connect(url)
                .await
                .map_err(|e| Error::Generic(e.to_string()))?,
        })
    }
}

/// WebSocket client for non-WASM environments using tokio-tungstenite
#[cfg(all(feature = "amp0", not(target_arch = "wasm32")))]
pub struct WebSocketClient {
    write_stream: Arc<
        Mutex<
            futures::stream::SplitSink<
                tokio_tungstenite::WebSocketStream<
                    tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
                >,
                tokio_tungstenite::tungstenite::Message,
            >,
        >,
    >,
    read_stream: Arc<
        Mutex<
            futures::stream::SplitStream<
                tokio_tungstenite::WebSocketStream<
                    tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
                >,
            >,
        >,
    >,
}

#[cfg(all(feature = "amp0", not(target_arch = "wasm32")))]
impl WebSocketClient {
    /// Connect to a WebSocket URL
    pub async fn connect(url: &str) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        use futures::StreamExt;
        use tokio_tungstenite::connect_async;

        let (ws_stream, _) = connect_async(url).await?;
        let (write, read) = ws_stream.split();

        Ok(Self {
            write_stream: Arc::new(Mutex::new(write)),
            read_stream: Arc::new(Mutex::new(read)),
        })
    }

    /// Connect to a WebSocket URL with a specific protocol
    pub async fn connect_with_protocol(
        url: &str,
        protocol: &str,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        use futures::StreamExt;
        use tokio_tungstenite::{
            connect_async_with_config,
            tungstenite::{client::IntoClientRequest, http::HeaderValue},
        };

        // Start with the URL and let it create the base request
        let mut request = url.into_client_request()?;

        // Add the protocol header
        request
            .headers_mut()
            .insert("Sec-WebSocket-Protocol", HeaderValue::from_str(protocol)?);

        let (ws_stream, _) = connect_async_with_config(request, None, false).await?;
        let (write, read) = ws_stream.split();

        Ok(Self {
            write_stream: Arc::new(Mutex::new(write)),
            read_stream: Arc::new(Mutex::new(read)),
        })
    }

    /// Connect to a WebSocket URL with WAMP 2.0 JSON protocol
    pub async fn connect_wamp(url: &str) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        Self::connect_with_protocol(url, "wamp.2.json").await
    }

    /// Send a text message
    pub async fn send_text(
        &self,
        text: &str,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        use futures::SinkExt;
        use tokio_tungstenite::tungstenite::Message;

        let mut write_stream = self.write_stream.lock().await;
        write_stream.send(Message::Text(text.to_string())).await?;
        Ok(())
    }

    /// Send binary data
    pub async fn send_binary(
        &self,
        data: &[u8],
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        use futures::SinkExt;
        use tokio_tungstenite::tungstenite::Message;

        let mut write_stream = self.write_stream.lock().await;
        write_stream.send(Message::Binary(data.to_vec())).await?;
        Ok(())
    }
}

/// Custom error type for WebSocket operations
#[cfg(all(feature = "amp0", not(target_arch = "wasm32")))]
#[derive(Debug, thiserror::Error)]
pub enum WebSocketError {
    #[error("WebSocket error: {0}")]
    WebSocket(#[from] tokio_tungstenite::tungstenite::Error),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("UTF-8 error: {0}")]
    Utf8(#[from] std::str::Utf8Error),
    #[error("Connection closed")]
    ConnectionClosed,
    #[error("Invalid message type")]
    InvalidMessageType,
}

/// Implement the Stream trait for WebSocketClient
#[cfg(all(feature = "amp0", not(target_arch = "wasm32")))]
impl Stream for WebSocketClient {
    type Error = WebSocketError;

    async fn read(&self, buf: &mut [u8]) -> Result<usize, Self::Error> {
        use futures::StreamExt;
        use tokio_tungstenite::tungstenite::Message;

        let mut read_stream = self.read_stream.lock().await;

        loop {
            match read_stream.next().await {
                Some(Ok(Message::Binary(data))) => {
                    let len = std::cmp::min(buf.len(), data.len());
                    buf[..len].copy_from_slice(&data[..len]);
                    return Ok(len);
                }
                Some(Ok(Message::Text(text))) => {
                    let data = text.as_bytes();
                    let len = std::cmp::min(buf.len(), data.len());
                    buf[..len].copy_from_slice(&data[..len]);
                    return Ok(len);
                }
                Some(Ok(Message::Close(_))) => {
                    return Err(WebSocketError::ConnectionClosed);
                }
                Some(Ok(_)) => {
                    // Skip ping/pong frames
                    continue;
                }
                Some(Err(e)) => {
                    return Err(WebSocketError::WebSocket(e));
                }
                None => {
                    return Err(WebSocketError::ConnectionClosed);
                }
            }
        }
    }

    async fn write(&self, data: &[u8]) -> Result<(), Self::Error> {
        use futures::SinkExt;
        use tokio_tungstenite::tungstenite::Message;

        let mut write_stream = self.write_stream.lock().await;

        // Try to send as text first (for JSON protocols), fall back to binary
        if let Ok(text) = std::str::from_utf8(data) {
            write_stream.send(Message::Text(text.to_string())).await?;
        } else {
            write_stream.send(Message::Binary(data.to_vec())).await?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Test WebSocket connection to Blockstream's Green Liquid mainnet endpoint
    /// This test demonstrates connecting to a real WebSocket server with WAMP protocol
    #[cfg(all(feature = "amp0", not(target_arch = "wasm32")))]
    #[tokio::test]
    #[ignore] // Requires network connectivity
    async fn test_websocket_client_wamp_connection() {
        let client =
            WebSocketClient::connect_wamp("wss://green-liquid-mainnet.blockstream.com/v2/ws/")
                .await
                .expect("Failed to connect to WebSocket");

        // WAMP HELLO message
        let hello_message = r#"[1, "realm1", {"roles": {"caller": {"features": {}}}}]"#;

        // Send HELLO message using Stream trait
        let hello_bytes = hello_message.as_bytes();
        client
            .write(hello_bytes)
            .await
            .expect("Failed to send HELLO message");

        // Read response using Stream trait
        let mut response_buffer = vec![0u8; 4096];
        let bytes_read = client
            .read(&mut response_buffer)
            .await
            .expect("Failed to read response");

        // Convert response to string and verify it's a WELCOME message
        let response_str = String::from_utf8_lossy(&response_buffer[..bytes_read]);
        println!("Received response: {}", response_str);

        // Parse as JSON and verify structure
        let response_json: serde_json::Value =
            serde_json::from_str(&response_str).expect("Failed to parse response as JSON");

        if let serde_json::Value::Array(ref arr) = response_json {
            assert!(
                arr.len() >= 3,
                "WELCOME message should have at least 3 elements"
            );
            assert_eq!(
                arr[0], 2,
                "First element should be 2 (WELCOME message type)"
            );
            assert!(arr[1].is_number(), "Second element should be session ID");
            assert!(arr[2].is_object(), "Third element should be details object");
        } else {
            panic!("Response should be a JSON array");
        }
    }

    /// Test WebSocket client creation (without network)
    #[cfg(all(feature = "amp0", not(target_arch = "wasm32")))]
    #[tokio::test]
    async fn test_websocket_client_creation() {
        // This test will fail since the URL doesn't exist, but it tests the API
        let result = WebSocketClient::connect("ws://localhost:1234").await;
        assert!(
            result.is_err(),
            "Connection should fail for non-existent URL"
        );
    }
}
