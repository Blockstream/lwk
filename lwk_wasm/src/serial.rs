use lwk_common::Stream;
use serde::Serialize;
use wasm_bindgen::{JsCast, JsValue};
use web_sys::js_sys::Uint8Array;

use crate::Error;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SerialFilter {
    usb_vendor_id: u16,
    usb_product_id: u16,
}

fn populate_filters(options: &mut web_sys::SerialPortRequestOptions) {
    let ids: Vec<_> = lwk_jade::JADE_DEVICE_IDS
        .iter()
        .map(|ids| SerialFilter {
            usb_vendor_id: ids.0,
            usb_product_id: ids.1,
        })
        .collect();
    let ids = serde_wasm_bindgen::to_value(&ids).expect("static");
    options.set_filters(&ids);
}

pub(crate) async fn get_jade_serial(filter: bool) -> Result<web_sys::SerialPort, Error> {
    let window =
        web_sys::window().ok_or_else(|| Error::Generic("cannot get window".to_string()))?;
    let navigator = window.navigator();

    let has_serial = web_sys::js_sys::Reflect::get(&navigator, &"serial".into())
        .map(|val| !val.is_undefined())
        .unwrap_or(false);
    if !has_serial {
        let msg = "The used browser doesn't support web serial".to_string();
        return Err(Error::Generic(msg));
    }

    let serial = navigator.serial();

    // Check if we already have ports with permission granted
    let result = wasm_bindgen_futures::JsFuture::from(serial.get_ports())
        .await
        .map_err(Error::JsVal)?;
    let serials: web_sys::js_sys::Array = result.dyn_into().map_err(Error::JsVal)?;
    let result = if serials.length() > 0 {
        // TODO should check getPortInfo to see if it has right JADE_DEVICE_IDS
        serials.get(0)
    } else {
        let promise = if filter {
            let mut options = web_sys::SerialPortRequestOptions::new();
            populate_filters(&mut options);
            serial.request_port_with_options(&options)
        } else {
            serial.request_port()
        };

        wasm_bindgen_futures::JsFuture::from(promise)
            .await
            .map_err(Error::JsVal)?
    };

    let serial: web_sys::SerialPort = result.dyn_into().map_err(Error::JsVal)?;

    let serial_options = web_sys::SerialOptions::new(115_200);

    let promise = serial.open(&serial_options);
    wasm_bindgen_futures::JsFuture::from(promise)
        .await
        .map_err(Error::JsVal)?;

    Ok(serial)
}

pub struct WebSerial {
    reader: web_sys::ReadableStreamDefaultReader,
    writer: web_sys::WritableStreamDefaultWriter,
}
impl WebSerial {
    pub fn new(serial_port: &web_sys::SerialPort) -> Result<Self, Error> {
        Ok(Self {
            reader: web_sys::ReadableStreamDefaultReader::new(&serial_port.readable())
                .map_err(Error::JsVal)?,
            writer: serial_port.writable().get_writer().map_err(Error::JsVal)?,
        })
    }
}

impl Stream for WebSerial {
    type Error = lwk_jade::Error;

    async fn read(&self, buf: &mut [u8]) -> Result<usize, lwk_jade::Error> {
        let promise = self.reader.read();
        let result = wasm_bindgen_futures::JsFuture::from(promise)
            .await
            .map_err(generic_jade_err)?;
        let value =
            web_sys::js_sys::Reflect::get(&result, &"value".into()).map_err(generic_jade_err)?;
        let data = web_sys::js_sys::Uint8Array::new(&value).to_vec();
        buf[..data.len()].copy_from_slice(&data);
        Ok(data.len())
    }

    async fn write(&self, buf: &[u8]) -> Result<(), lwk_jade::Error> {
        let arr = Uint8Array::new_with_length(buf.len() as u32);
        arr.copy_from(buf);
        let promise = self.writer.write_with_chunk(&arr);
        wasm_bindgen_futures::JsFuture::from(promise)
            .await
            .map_err(generic_jade_err)?;
        Ok(())
    }
}

fn generic_jade_err(val: JsValue) -> lwk_jade::Error {
    lwk_jade::Error::Generic(format!("{:?}", val))
}
