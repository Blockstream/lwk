use crate::Error;
use lwk_ledger::asyncr::Ledger;
use lwk_ledger::asyncr::LiquidClient;
use lwk_ledger::{APDUCmdVec, StatusWord};
use serde::Serialize;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::HidDevice;
use web_sys::HidDeviceRequestOptions;

const LEDGER_DEVICE_IDS: [(u16, u16); 1] = [(0x2c97, 0x1011)];

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct HidFilter {
    vendor_id: u16,
    product_id: u16,
}

struct TransportWeb {
    hid_device: HidDevice,
}

#[wasm_bindgen]
struct LedgerWeb {
    ledger: Ledger<TransportWeb>,
}

// https://github.com/LedgerHQ/ledger-live/blob/develop/libs/ledgerjs/packages/devices/src/hid-framing.ts
// https://github.com/LedgerHQ/ledger-live/blob/8fe361435ef6eef06fa028845977369990f36f71/libs/ledgerjs/packages/hw-transport-webhid/src/TransportWebHID.ts

#[wasm_bindgen]
impl LedgerWeb {
    /// hid_device must be already opened
    #[wasm_bindgen(constructor)]
    pub fn new(hid_device: HidDevice) -> Self {
        let transport = TransportWeb { hid_device };

        let ledger = Ledger::from_transport(transport);
        Self { ledger }
    }

    pub async fn get_version(&self) -> Result<String, Error> {
        let (a, b, c) = self
            .ledger
            .client
            .get_version()
            .await
            .map_err(|_| Error::Generic("error getting version".to_string()))?;
        Ok(format!("{} {} {:?}", a, b, c))
    }
}

impl lwk_ledger::asyncr::Transport for TransportWeb {
    type Error = Error;

    async fn exchange(&self, command: &APDUCmdVec) -> Result<(StatusWord, Vec<u8>), Self::Error> {
        let mut data = command.serialize();
        let i = JsValue::from_f64(data.len() as f64);
        web_sys::console::log_2(&"data len".into(), &i);

        let f = move |e: web_sys::HidInputReportEvent| {
            web_sys::console::log_2(&"HidInputReportEvent".into(), &e);

            let dataview = e.data();
            // log::debug!("  DataView: {:?}", &dataview);
            let ofs = dataview.byte_offset();
            let len = dataview.byte_length();
            let ba: Vec<u8> = (0..len).map(|i| dataview.get_uint8(i + ofs)).collect();
            // log::debug!("  Vec<u8>: {:?}", &ba);
            let i = JsValue::from_f64(ba.len() as f64);
            web_sys::console::log_2(&"ba len".into(), &i);

            // TODO how do I get the result?
        };
        let closure: Closure<dyn FnMut(_)> = Closure::once(f);

        // // https://gist.github.com/kndysfm/f722e2b6dc26ab28e3da5945d5e21933

        self.hid_device
            .set_oninputreport(Some(closure.as_ref().unchecked_ref()));

        let report_id = 0x00;
        let promise = self
            .hid_device
            .send_report_with_u8_slice(report_id, &mut data)
            .unwrap();
        web_sys::console::log_2(&"promise22".into(), &promise);

        let result = wasm_bindgen_futures::JsFuture::from(promise)
            .await
            .map_err(Error::JsVal)?;
        web_sys::console::log_2(&"result33".into(), &result);

        lwk_wollet::clients::asyncr::async_sleep(100).await;

        // let i = JsValue::from_f64(output.len() as f64);
        let res = closure.into_js_value();
        web_sys::console::log_2(&"closure".into(), &res);

        Ok((StatusWord::OK, vec![]))
    }
}

//https://rustwasm.github.io/wasm-bindgen/api/web_sys/struct.HidDevice.html#method.send_report_with_u8_slice
#[wasm_bindgen]
pub async fn search_ledger_device() -> Result<HidDevice, Error> {
    let window =
        web_sys::window().ok_or_else(|| Error::Generic("cannot get window".to_string()))?;
    let navigator = window.navigator();

    let has_hid = web_sys::js_sys::Reflect::get(&navigator, &"hid".into())
        .map(|val| !val.is_undefined())
        .unwrap_or(false);
    if !has_hid {
        let msg = "The used browser doesn't support hid".to_string();
        return Err(Error::Generic(msg));
    }
    let navigator_hid = navigator.hid();

    // Check if we already have ports with permission granted
    let result = wasm_bindgen_futures::JsFuture::from(navigator_hid.get_devices())
        .await
        .map_err(Error::JsVal)?;
    let hids: web_sys::js_sys::Array = result.dyn_into().map_err(Error::JsVal)?;
    web_sys::console::log_2(&"hids".into(), &hids);

    let hid_device = if hids.length() > 0 {
        // TODO handle multiple ledgers?
        js_val_to_hid_device(hids.get(0))?
    } else {
        let ids: Vec<_> = LEDGER_DEVICE_IDS
            .iter()
            .map(|ids| HidFilter {
                vendor_id: ids.0,
                product_id: ids.1,
            })
            .collect();
        let ids = serde_wasm_bindgen::to_value(&ids).expect("static");

        let options = HidDeviceRequestOptions::new(&ids);
        let result = wasm_bindgen_futures::JsFuture::from(navigator_hid.request_device(&options))
            .await
            .map_err(Error::JsVal)?;

        let hids: web_sys::js_sys::Array = result.dyn_into().map_err(Error::JsVal)?;
        web_sys::console::log_2(&"hids".into(), &hids);

        if hids.length() > 0 {
            // TODO handle multiple ledgers?
            js_val_to_hid_device(hids.get(0))?
        } else {
            return Err(Error::Generic("no ledger found".to_string()));
        }
    };

    if !hid_device.opened() {
        let promise = hid_device.open();
        let result = wasm_bindgen_futures::JsFuture::from(promise)
            .await
            .map_err(Error::JsVal)?;
        web_sys::console::log_2(&"device was opened".into(), &result);
    }
    Ok(hid_device)
}

fn js_val_to_hid_device(js_val: JsValue) -> Result<HidDevice, Error> {
    let hid = js_val.dyn_into::<HidDevice>().map_err(Error::JsVal)?;
    Ok(hid)
}
