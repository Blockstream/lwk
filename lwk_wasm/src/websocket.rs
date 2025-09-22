use crate::Error;
use lwk_common::Stream;
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::js_sys;
use web_sys::{BinaryType, Event, MessageEvent, WebSocket};

/// WebSocket-based implementation of the Stream trait
///
/// Supports both binary mode (for protocols like Jade) and text mode (for protocols like WAMP).
/// The mode is set during construction and determines how messages are sent and received.
pub struct WebSocketSerial {
    websocket: WebSocket,
    receive_buffer: Arc<Mutex<VecDeque<u8>>>,
    text_mode: bool,
}

impl WebSocketSerial {
    pub async fn new(url: &str) -> Result<Self, Error> {
        Self::new_with_protocol(url, None, false).await
    }

    pub async fn new_binary(url: &str) -> Result<Self, Error> {
        Self::new_with_protocol(url, None, false).await
    }

    pub async fn new_text(url: &str) -> Result<Self, Error> {
        Self::new_with_protocol(url, None, true).await
    }

    pub async fn new_with_protocol(
        url: &str,
        protocol: Option<&str>,
        text_mode: bool,
    ) -> Result<Self, Error> {
        let websocket = if let Some(protocol) = protocol {
            // Create array with single protocol
            let protocol_array = js_sys::Array::new();
            protocol_array.push(&JsValue::from_str(protocol));
            WebSocket::new_with_str_sequence(url, &protocol_array).map_err(Error::JsVal)?
        } else {
            WebSocket::new(url).map_err(Error::JsVal)?
        };
        websocket.set_binary_type(BinaryType::Arraybuffer);

        let receive_buffer = Arc::new(Mutex::new(VecDeque::new()));
        let buffer_clone = receive_buffer.clone();

        // Set up message handler based on mode
        let onmessage_callback = if text_mode {
            // Text mode: handle text messages, ignore binary
            Closure::wrap(Box::new(move |e: MessageEvent| {
                if let Ok(text) = e.data().dyn_into::<js_sys::JsString>() {
                    let text_str: String = text.into();
                    let data = text_str.as_bytes();
                    if let Ok(mut buffer) = buffer_clone.lock() {
                        buffer.extend(data);
                    }
                }
            }) as Box<dyn FnMut(_)>)
        } else {
            // Binary mode: handle binary messages, ignore text
            Closure::wrap(Box::new(move |e: MessageEvent| {
                if let Ok(array_buffer) = e.data().dyn_into::<web_sys::js_sys::ArrayBuffer>() {
                    let uint8_array = web_sys::js_sys::Uint8Array::new(&array_buffer);
                    let data = uint8_array.to_vec();
                    if let Ok(mut buffer) = buffer_clone.lock() {
                        buffer.extend(data);
                    }
                }
            }) as Box<dyn FnMut(_)>)
        };

        websocket.set_onmessage(Some(onmessage_callback.as_ref().unchecked_ref()));
        onmessage_callback.forget();

        // Wait for connection to open
        let connection_promise = web_sys::js_sys::Promise::new(&mut |resolve, _reject| {
            let onopen_callback = Closure::wrap(Box::new(move |_event: Event| {
                resolve.call0(&JsValue::NULL).unwrap();
            }) as Box<dyn FnMut(Event)>);

            websocket.set_onopen(Some(onopen_callback.as_ref().unchecked_ref()));
            onopen_callback.forget();
        });

        wasm_bindgen_futures::JsFuture::from(connection_promise)
            .await
            .map_err(Error::JsVal)?;

        Ok(Self {
            websocket,
            receive_buffer,
            text_mode,
        })
    }

    /// Create a WebSocket connection with WAMP 2.0 JSON protocol
    /// This is a convenience method for WAMP connections (uses text mode)
    pub async fn new_wamp(url: &str) -> Result<Self, Error> {
        Self::new_with_protocol(url, Some("wamp.2.json"), true).await
    }

    /// Send a text message directly (useful for JSON/WAMP messages)
    /// This method works regardless of the WebSocket's text/binary mode setting
    pub async fn send_text(&self, text: &str) -> Result<(), lwk_jade::Error> {
        self.websocket
            .send_with_str(text)
            .map_err(|e| lwk_jade::Error::Generic(format!("WebSocket send error: {:?}", e)))?;
        Ok(())
    }

    /// Get a reference to the underlying WebSocket
    pub fn websocket(&self) -> &WebSocket {
        &self.websocket
    }
}

impl Stream for WebSocketSerial {
    type Error = lwk_jade::Error;

    async fn read(&self, buf: &mut [u8]) -> Result<usize, lwk_jade::Error> {
        // Try to read from buffer first
        loop {
            {
                let mut buffer = self
                    .receive_buffer
                    .lock()
                    .map_err(|e| lwk_jade::Error::Generic(format!("Mutex error: {}", e)))?;

                if !buffer.is_empty() {
                    let read_len = std::cmp::min(buf.len(), buffer.len());
                    for i in 0..read_len {
                        buf[i] = buffer.pop_front().unwrap();
                    }
                    return Ok(read_len);
                }
            }

            // Cross-platform timeout
            let global_obj = js_sys::global();
            let set_timeout = js_sys::Reflect::get(&global_obj, &"setTimeout".into())
                .map_err(|e| {
                    lwk_jade::Error::Generic(format!("Failed to get setTimeout: {:?}", e))
                })?
                .dyn_into::<js_sys::Function>()
                .map_err(|e| {
                    lwk_jade::Error::Generic(format!("setTimeout not a function: {:?}", e))
                })?;

            let promise = js_sys::Promise::new(&mut |resolve, _reject| {
                set_timeout
                    .call2(&JsValue::NULL, &resolve, &JsValue::from_f64(10.0))
                    .map_err(|e| {
                        lwk_jade::Error::Generic(format!("setTimeout call failed: {:?}", e))
                    })
                    .unwrap();
            });

            wasm_bindgen_futures::JsFuture::from(promise)
                .await
                .map_err(|e| lwk_jade::Error::Generic(format!("Timeout error: {:?}", e)))?;
        }
    }

    async fn write(&self, buf: &[u8]) -> Result<(), lwk_jade::Error> {
        if self.text_mode {
            // Text mode: convert bytes to string and send as text
            let text = std::str::from_utf8(buf)
                .map_err(|e| lwk_jade::Error::Generic(format!("Invalid UTF-8: {}", e)))?;
            self.websocket
                .send_with_str(text)
                .map_err(|e| lwk_jade::Error::Generic(format!("WebSocket send error: {:?}", e)))?;
        } else {
            // Binary mode: send as binary data
            let uint8_array = web_sys::js_sys::Uint8Array::new_with_length(buf.len() as u32);
            uint8_array.copy_from(buf);

            self.websocket
                .send_with_array_buffer(&uint8_array.buffer())
                .map_err(|e| lwk_jade::Error::Generic(format!("WebSocket send error: {:?}", e)))?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wasm_bindgen_test::*;

    wasm_bindgen_test_configure!(run_in_browser);

    /// Test WebSocket connection to Blockstream's Green Liquid mainnet endpoint
    /// Mimics: echo '[1, "realm1", {"roles": {"caller": {"features": {}}}}]' | websocat --insecure --protocol wamp.2.json wss://green-liquid-mainnet.blockstream.com/v2/ws/
    ///
    /// Note: Uses the "wamp.2.json" WebSocket subprotocol as required by the server.
    /// This test is ignored by default as it requires network connectivity.
    /// Run with: wasm-pack test --headless --chrome -- --ignored
    #[wasm_bindgen_test]
    async fn test_blockstream_green_websocket_connection() {
        console_error_panic_hook::set_once();

        web_sys::console::log_1(&"Connecting to WebSocket...".into());

        // Connect to Blockstream Green WebSocket endpoint with WAMP protocol
        let ws_serial =
            WebSocketSerial::new_wamp("wss://green-liquid-mainnet.blockstream.com/v2/ws/")
                .await
                .expect("Failed to connect to WebSocket");

        web_sys::console::log_1(&"Connected! Sending HELLO message...".into());

        // WAMP HELLO message: [1, "realm1", {"roles": {"caller": {"features": {}}}}]
        let hello_message = r#"[1, "realm1", {"roles": {"caller": {"features": {}}}}]"#;

        // Send HELLO message as text (WAMP uses JSON text messages)
        ws_serial
            .send_text(hello_message)
            .await
            .expect("Failed to send HELLO message");

        web_sys::console::log_1(&"HELLO message sent, waiting for response...".into());

        // Read response (WELCOME message) with timeout protection
        let mut response_buffer = vec![0u8; 4096];
        let mut attempts = 0;
        let max_attempts = 100; // 1 second timeout (10ms * 100)

        let bytes_read = loop {
            match ws_serial.read(&mut response_buffer).await {
                Ok(bytes) if bytes > 0 => break bytes,
                Ok(_) => {
                    attempts += 1;
                    if attempts >= max_attempts {
                        panic!("Timeout waiting for WELCOME message");
                    }
                }
                Err(e) => panic!("Failed to read WELCOME message: {:?}", e),
            }
        };

        web_sys::console::log_1(&format!("Received {} bytes", bytes_read).into());

        // Convert response to string
        let response_str = String::from_utf8_lossy(&response_buffer[..bytes_read]);
        web_sys::console::log_1(&format!("Received response: {}", response_str).into());

        // Parse as JSON to validate structure
        let response_json: serde_json::Value =
            serde_json::from_str(&response_str).expect("Failed to parse response as JSON");

        // Validate it's a WELCOME message (message type 2)
        if let serde_json::Value::Array(ref arr) = response_json {
            assert!(
                arr.len() >= 3,
                "WELCOME message should have at least 3 elements"
            );
            assert_eq!(
                arr[0], 2,
                "First element should be 2 (WELCOME message type)"
            );

            // Second element should be session ID (number)
            assert!(
                arr[1].is_number(),
                "Second element should be session ID (number)"
            );

            // Third element should be details object
            assert!(arr[2].is_object(), "Third element should be details object");

            let details = &arr[2];
            assert!(details["realm"].is_string(), "Details should contain realm");
            assert!(
                details["authid"].is_string(),
                "Details should contain authid"
            );
            assert!(
                details["authrole"].is_string(),
                "Details should contain authrole"
            );
            assert!(details["roles"].is_object(), "Details should contain roles");
        } else {
            panic!("Response should be a JSON array");
        }

        web_sys::console::log_1(&"WebSocket connection test completed successfully!".into());
    }
}
