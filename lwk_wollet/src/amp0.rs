use aes::cipher::{block_padding::Pkcs7, BlockDecryptMut, KeyIvInit};
use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Key, Nonce,
};
use base64::prelude::*;
use elements::bitcoin::bip32::Xpub;
use flate2::read::ZlibDecoder;
use hmac::Hmac;
use lwk_common::{Network, Stream};
use pbkdf2::pbkdf2;
use rmpv;
use scrypt::{scrypt, Params};
use serde::{Deserialize, Serialize};
use sha2::Sha512;
use std::collections::HashMap;
use std::io::Read;

#[cfg(all(feature = "amp0", not(target_arch = "wasm32")))]
use std::sync::Arc;
#[cfg(all(feature = "amp0", not(target_arch = "wasm32")))]
use tokio::sync::Mutex;

use crate::{hex, Error};

pub struct Amp0<S: Stream> {
    stream: S,
}

pub const WATCH_ONLY_SALT: [u8; 8] = [0x5f, 0x77, 0x6f, 0x5f, 0x73, 0x61, 0x6c, 0x74]; // '_wo_salt'
pub const WO_SEED_U: [u8; 8] = [0x01, 0x77, 0x6f, 0x5f, 0x75, 0x73, 0x65, 0x72]; // [1]'wo_user'
pub const WO_SEED_P: [u8; 8] = [0x02, 0x77, 0x6f, 0x5f, 0x70, 0x61, 0x73, 0x73]; // [2]'wo_pass'
pub const WO_SEED_K: [u8; 8] = [0x03, 0x77, 0x6f, 0x5f, 0x62, 0x6C, 0x6f, 0x62]; // [3]'wo_blob'

/// Green subaccount data returned at login
#[derive(Debug, Deserialize, Serialize)]
pub struct GreenSubaccount {
    /// Subaccount pointer
    pub pointer: u32,

    /// Subaccount type
    ///
    /// We're only interested in type "2of2_no_recovery"
    #[serde(rename = "type")]
    pub type_: String,

    /// Green Address ID, aka AMP ID
    #[serde(rename = "receiving_id")]
    pub gaid: String,

    /// Number of confidential addresses that should be uploaded for this subaccounts
    pub required_ca: u32,
}

/// Login Data returned by Green backend
///
/// Only the content that we use
#[derive(Debug, Deserialize, Serialize)]
pub struct LoginData {
    /// Derivation path used to derive the Green server xpub
    ///
    /// 128 hex chars
    pub gait_path: String,

    /// Key used to encrypt/decrypt the blob
    ///
    /// 128 hex chars
    /// Note: this key is itself encrypted
    pub wo_blob_key: String,

    /// Wallet subaccounts
    pub subaccounts: Vec<GreenSubaccount>,
}

impl<S: Stream> Amp0<S> {
    pub async fn new(stream: S) -> Result<Self, Error> {
        Ok(Self { stream })
    }

    /// Login to the Green Address API with clear credentials performing the hashing internally.
    pub async fn login(
        &self,
        clear_username: &str,
        clear_password: &str,
    ) -> Result<LoginData, Error> {
        let (hashed_username, hashed_password) =
            encrypt_credentials(clear_username, clear_password);
        self.login_with_hashed_credentials(&hashed_username, &hashed_password)
            .await
    }

    /// Login to the Green Address API with pre-hashed credentials
    ///
    /// This method takes already hashed username and password. Since username and password
    /// hashing is computationally heavy (requires hundreds of milliseconds), it's recommended
    /// to use [`encrypt_credentials()`] to hash the username and password once and cache the
    /// result for subsequent logins to improve performance.
    ///
    /// For convenience, use [`Self::login`] to automatically hash clear credentials.
    pub async fn login_with_hashed_credentials(
        &self,
        hashed_username: &str,
        hashed_password: &str,
    ) -> Result<LoginData, Error> {
        // Step 1: Send WAMP HELLO message
        let hello_msg = r#"[1, "realm1", {"roles": {"caller": {"features": {}}}}]"#;
        self.stream
            .write(hello_msg.as_bytes())
            .await
            .map_err(|e| Error::Generic(format!("Failed to send HELLO: {}", e)))?;

        // Step 2: Wait for WELCOME response
        let mut buf = vec![0u8; 10000];

        #[cfg(not(target_arch = "wasm32"))]
        let bytes_read = tokio::time::timeout(
            std::time::Duration::from_secs(10),
            self.stream.read(&mut buf),
        )
        .await
        .map_err(|_| Error::Generic("WELCOME timeout after 10 seconds".to_string()))?
        .map_err(|e| Error::Generic(format!("Failed to read WELCOME: {}", e)))?;

        #[cfg(target_arch = "wasm32")]
        let bytes_read = self
            .stream
            .read(&mut buf)
            .await
            .map_err(|e| Error::Generic(format!("Failed to read WELCOME: {}", e)))?;

        let welcome_response = String::from_utf8_lossy(&buf[..bytes_read]);

        // Verify it's a WELCOME message (should start with [2,...)
        if !welcome_response.trim_start().starts_with("[2,") {
            return Err(Error::Generic(format!(
                "Expected WELCOME message, got: {}",
                welcome_response
            )));
        }

        // Step 3: Send login call
        let login_msg = serde_json::json!([
            48,
            1,
            {},
            "com.greenaddress.login.watch_only_v2",
            [
                "custom",
                {
                    "username": hashed_username,
                    "password": hashed_password,
                    "minimal": "true"
                },
                "[v2,sw,csv,csv_opt]48c4e352e3add7ef3ae904b0acd15cf5fe2c5cc3",
                true
            ]
        ]);

        self.stream
            .write(login_msg.to_string().as_bytes())
            .await
            .map_err(|e| Error::Generic(format!("Failed to send login: {}", e)))?;

        // Step 4: Wait for login response (success or error)
        let mut response_buf = vec![0u8; 10000];

        #[cfg(not(target_arch = "wasm32"))]
        let response_bytes = tokio::time::timeout(
            std::time::Duration::from_secs(10),
            self.stream.read(&mut response_buf),
        )
        .await
        .map_err(|_| Error::Generic("Login response timeout after 10 seconds".to_string()))?
        .map_err(|e| Error::Generic(format!("Failed to read login response: {}", e)))?;

        #[cfg(target_arch = "wasm32")]
        let response_bytes = self
            .stream
            .read(&mut response_buf)
            .await
            .map_err(|e| Error::Generic(format!("Failed to read login response: {}", e)))?;

        let login_response = String::from_utf8_lossy(&response_buf[..response_bytes]);
        let err = Error::Generic(format!(
            "Unexpected login data response: {}",
            login_response
        ));

        // Login response has this format
        // [
        //   50,
        //   1,
        //   {...},
        //   [login_data]
        // ]
        // TODO: improve parsing of wamp msg pack responses
        let Ok(v) = serde_json::from_str::<serde_json::Value>(&login_response) else {
            return Err(err);
        };
        let Some(v) = v.as_array() else {
            return Err(err);
        };
        let [_, _, _, ref v] = v[..] else {
            return Err(err);
        };
        let Some(v) = v.as_array() else {
            return Err(err);
        };
        let [ref v] = v[..] else {
            return Err(err);
        };
        serde_json::from_value(v.clone()).map_err(|_| err)
    }
}

pub fn get_entropy(username: &str, password: &str) -> [u8; 64] {
    // https://gl.blockstream.io/blockstream/green/gdk/-/blame/master/src/utils.cpp#L334
    let salt_string: &[u8] = &WATCH_ONLY_SALT;

    let u_p = format!("{}{}", username, password);
    let mut entropy = vec![0u8; 4 + u_p.len()];

    // Write username length as 32-bit integer
    let username_len = username.len() as u32;
    entropy[0..4].copy_from_slice(&username_len.to_le_bytes());

    // Copy concatenated username and password
    entropy[4..].copy_from_slice(u_p.as_bytes());

    let mut output = [0u8; 64];
    let params = Params::new(
        14, // log_n (2^14 = 16384 iterations)
        8,  // r (block size)
        8,  // p (parallelization)
        64, // output length in bytes
    );
    scrypt(
        &entropy,
        salt_string,
        &params.expect("script parameters defined statically"),
        &mut output,
    )
    .expect("`output.len() > 0 && output.len() <= (2^32 - 1) * 32`.");

    output
}

pub fn encrypt_credentials(username: &str, password: &str) -> (String, String) {
    let entropy = get_entropy(username, password);

    // https://gl.blockstream.io/blockstream/green/gdk/-/blame/master/src/ga_session.cpp#L222

    // Calculate u_blob and p_blob using PBKDF2-HMAC-SHA512-256
    let mut u_blob = [0u8; 32];
    let mut p_blob = [0u8; 32];

    let _ = pbkdf2::<Hmac<Sha512>>(&entropy, &WO_SEED_U, 2048, &mut u_blob);
    let _ = pbkdf2::<Hmac<Sha512>>(&entropy, &WO_SEED_P, 2048, &mut p_blob);

    (hex::encode(&u_blob), hex::encode(&p_blob))
}

pub fn decrypt_blob_key(
    username: &str,
    password: &str,
    wo_blob_key_hex: &str,
) -> Result<Vec<u8>, Error> {
    let entropy = get_entropy(username, password);
    let mut wo_aes_key = [0u8; 32];
    let _ = pbkdf2::<Hmac<Sha512>>(&entropy, &WO_SEED_K, 2048, &mut wo_aes_key);

    let data = hex::_decode(wo_blob_key_hex)?;

    let iv: [u8; 16] = data[..16]
        .try_into()
        .map_err(|_| Error::Generic("Invalid IV".to_string()))?;
    let enc_key = cbc::Decryptor::<aes::Aes256>::new(&wo_aes_key.into(), &iv.into())
        .decrypt_padded_vec_mut::<Pkcs7>(&data[16..])
        .map_err(|e| Error::Generic(e.to_string()))?;

    Ok(enc_key)
}

pub fn decrypt_blob(enc_key: &[u8], blob64: &str) -> Result<Vec<u8>, Error> {
    let wo_blob = BASE64_STANDARD
        .decode(blob64)
        .map_err(|e| Error::Generic(e.to_string()))?;

    if enc_key.len() != 32 {
        return Err(Error::Generic("Invalid encryption key length".into()));
    }
    // panicks on length mismatch
    let key = Key::<Aes256Gcm>::from_slice(enc_key);
    let cipher = Aes256Gcm::new(key);

    let nonce: [u8; 12] = wo_blob[..12]
        .try_into()
        .map_err(|_| Error::Generic("Invalid nonce".to_string()))?;
    let nonce = Nonce::from_slice(&nonce);
    let plaintext = cipher.decrypt(nonce, &wo_blob[12..])?;
    // plaintext should start with [1, 0, 0, 0] but it's not worth checking it here
    // as it might break after if someone sets the blob without this prefix
    Ok(plaintext)
}

pub fn parse_value(blob: &[u8]) -> Result<rmpv::Value, Error> {
    // decompress
    // bytes 0 to 4 are prefix
    // bytes 4 to 8 are ignored
    let mut d = ZlibDecoder::new(&blob[8..]);
    let mut v = vec![];
    d.read_to_end(&mut v)?;

    // messagePack to json
    let mut cursor = std::io::Cursor::new(v);
    let value = rmpv::decode::read_value(&mut cursor)?;
    Ok(value)
}

/// Useful content from the client blob
pub struct BlobContent {
    /// Wallet "master"/descriptor blinding key (always slip77)
    pub slip77_key: String,

    /// Wallet xpubs and their derivation paths
    pub xpubs: HashMap<Xpub, Vec<u32>>,
}

pub fn default_url(network: Network) -> Result<&'static str, Error> {
    match network {
        Network::Liquid => Ok("wss://green-liquid-mainnet.blockstream.com/v2/ws/"),
        Network::TestnetLiquid => Ok("wss://green-liquid-testnet.blockstream.com/v2/ws/"),
        Network::LocaltestLiquid => Ok("ws://localhost:8080/v2/ws"),
    }
}

#[cfg(all(feature = "amp0", not(target_arch = "wasm32")))]
impl Amp0<WebSocketClient> {
    pub async fn with_network(network: Network) -> Result<Self, Error> {
        let url = default_url(network)?;

        Ok(Self {
            stream: WebSocketClient::connect_wamp(url)
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

    /// Test Amp0 login functionality with proper WAMP protocol flow
    /// This test demonstrates the complete WAMP handshake + login flow
    #[cfg(all(feature = "amp0", not(target_arch = "wasm32")))]
    #[tokio::test]
    #[ignore] // Requires network connectivity
    async fn test_amp0_fail_login() {
        let amp0 = Amp0::with_network(Network::Liquid)
            .await
            .expect("Failed to connect to WebSocket");

        // Test with invalid credentials - should get proper error response, not timeout
        let response = amp0
            .login("invalid-user", "invalid-password")
            .await
            .unwrap_err();

        // Should get an error response like: [8,48,1,{},"com.greenaddress.error",["http://greenaddressit.com/error#usernotfound","User not found or invalid password",{}]]
        let response_str = format!("{:?}", response);
        assert!(!response_str.is_empty(), "Response should not be empty");
        assert!(
            response_str.contains("com.greenaddress.error") || response_str.contains("error"),
            "Response should contain error information, got: {}",
            response_str
        );
    }

    #[cfg(all(feature = "amp0", not(target_arch = "wasm32")))]
    #[tokio::test]
    #[ignore] // Requires network connectivity
    async fn test_amp0_ok_login() {
        let amp0 = Amp0::with_network(Network::Liquid)
            .await
            .expect("Failed to connect to WebSocket");

        let response = amp0
            .login("userleo456", "userleo456")
            .await
            .expect("Should get a response (even if it's an error)");
        println!("{:?}", response);

        assert_eq!(response.gait_path.len(), 128);
        assert_eq!(response.wo_blob_key.len(), 128);
        assert_eq!(response.subaccounts.len(), 1);
        assert_eq!(response.subaccounts[0].type_, "2of2_no_recovery");
        assert_eq!(response.subaccounts[0].pointer, 1);
        assert_eq!(
            response.subaccounts[0].gaid,
            "GA2zxWdhAYtREeYCVFTGRhHQmYMPAP"
        );
    }
}
