use crate::Error;
use lwk_common::Stream;
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{BinaryType, Event, MessageEvent, WebSocket};

/// WebSocket-based implementation of the Stream trait for Jade communication
pub struct WebSocketSerial {
    websocket: WebSocket,
    receive_buffer: Arc<Mutex<VecDeque<u8>>>,
}

impl WebSocketSerial {
    pub async fn new(url: &str) -> Result<Self, Error> {
        let websocket = WebSocket::new(url).map_err(Error::JsVal)?;
        websocket.set_binary_type(BinaryType::Arraybuffer);

        let receive_buffer = Arc::new(Mutex::new(VecDeque::new()));
        let buffer_clone = receive_buffer.clone();

        // Set up message handler
        let onmessage_callback = Closure::wrap(Box::new(move |e: MessageEvent| {
            if let Ok(array_buffer) = e.data().dyn_into::<web_sys::js_sys::ArrayBuffer>() {
                let uint8_array = web_sys::js_sys::Uint8Array::new(&array_buffer);
                let data = uint8_array.to_vec();
                if let Ok(mut buffer) = buffer_clone.lock() {
                    buffer.extend(data);
                }
            }
        }) as Box<dyn FnMut(_)>);

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
        })
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

            // Wait a bit for more data
            let promise = web_sys::js_sys::Promise::new(&mut |resolve, _reject| {
                web_sys::window()
                    .unwrap()
                    .set_timeout_with_callback_and_timeout_and_arguments_0(&resolve, 10)
                    .unwrap();
            });

            wasm_bindgen_futures::JsFuture::from(promise)
                .await
                .map_err(|e| lwk_jade::Error::Generic(format!("Timeout error: {:?}", e)))?;
        }
    }

    async fn write(&self, buf: &[u8]) -> Result<(), lwk_jade::Error> {
        let uint8_array = web_sys::js_sys::Uint8Array::new_with_length(buf.len() as u32);
        uint8_array.copy_from(buf);

        self.websocket
            .send_with_array_buffer(&uint8_array.buffer())
            .map_err(|e| lwk_jade::Error::Generic(format!("WebSocket send error: {:?}", e)))?;

        Ok(())
    }
}
