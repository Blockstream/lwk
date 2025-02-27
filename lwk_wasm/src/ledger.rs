use crate::Error;
use byteorder::BigEndian;
use byteorder::ReadBytesExt;
use lwk_ledger::asyncr::Ledger;
use lwk_ledger::asyncr::LiquidClient;
use lwk_ledger::read_multi_apdu;
use lwk_ledger::write_apdu;
use lwk_ledger::APDUAnswer;
use lwk_ledger::{APDUCmdVec, StatusWord};
use lwk_wollet::{bitcoin::bip32::DerivationPath, elements::pset::PartiallySignedTransaction};
use serde::Serialize;
use std::io::Cursor;
use std::str::FromStr;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::HidDevice;
use web_sys::HidDeviceRequestOptions;

const LEDGER_DEVICE_IDS: [(u16, u16); 1] = [(0x2c97, 0x1011)];

macro_rules! console_log {
    // Note that this is using the `log` function imported above during
    // `bare_bones`
    ($($t:tt)*) => (log(&format_args!($($t)*).to_string()))
}

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
// https://github.com/Zondax/ledger-rs/blob/master/ledger-transport-hid/src/lib.rs#L83C9-L83C15

#[wasm_bindgen]
impl LedgerWeb {
    /// hid_device must be already opened
    #[wasm_bindgen(constructor)]
    pub fn new(hid_device: HidDevice) -> Self {
        let transport = TransportWeb { hid_device };

        let ledger = Ledger::from_transport(transport);
        Self { ledger }
    }

    #[wasm_bindgen(js_name = getVersion)]
    pub async fn get_version(&self) -> Result<String, Error> {
        let (a, b, c) = self
            .ledger
            .client
            .get_version()
            .await
            .map_err(|e| Error::Generic(format!("{:?} error getting version", e)))?;
        console_log!("a {} b {} c {:?}", a, b, c);
        Ok(format!("{} {} {:?}", a, b, c))
    }

    #[wasm_bindgen(js_name = deriveXpub)]
    pub async fn derive_xpub(&self, path: &str) -> Result<String, Error> {
        let derivation_path = DerivationPath::from_str(&path).unwrap();
        let r = self
            .ledger
            .client
            .get_extended_pubkey(&derivation_path, false)
            .await
            .map_err(|e| {
                Error::Generic(format!(
                    "{:?} error getting XPUB derivation path {:?}.",
                    e, derivation_path
                ))
            })?;
        console_log!("r {}", r.to_string());
        Ok(r.to_string())
    }

    #[wasm_bindgen(js_name = slip77MasterBlindingKey)]
    pub async fn slip77_master_blinding_key(&self) -> Result<String, Error> {
        let r = self
            .ledger
            .client
            .get_master_blinding_key()
            .await
            .map_err(|e| Error::Generic(format!("{:?} error getting Master Blinding Key", e)))?;
        console_log!("r {}", r.to_string());
        Ok(r.to_string())
    }

    #[wasm_bindgen(js_name = fingerprint)]
    pub async fn fingerprint(&self) -> Result<String, Error> {
        let r = self
            .ledger
            .client
            .get_master_fingerprint()
            .await
            .map_err(|e| Error::Generic(format!("{:?} error getting fingerprint", e)))?;
        console_log!("r {}", r.to_string());
        Ok(r.to_string())
    }
}

impl lwk_ledger::asyncr::Transport for TransportWeb {
    type Error = Error;

    async fn exchange(&self, command: &APDUCmdVec) -> Result<(StatusWord, Vec<u8>), Self::Error> {
        let closure_result = std::rc::Rc::new(std::cell::RefCell::new(vec![]));

        let result_clone = closure_result.clone();

        let f = move |e: web_sys::HidInputReportEvent| {
            // TODO: how to handle multiple chunks?
            let dataview = e.data();

            let ofs = dataview.byte_offset();
            let len = dataview.byte_length();
            console_log!("ofs {} len {}", ofs, len);

            let ba: Vec<u8> = (0..len).map(|i| dataview.get_uint8(i + ofs)).collect();
            console_log!("ba {:?}", ba);

            let mut c = result_clone.borrow_mut();
            (*c).push(ba);
        };
        let closure: Closure<dyn FnMut(_)> = Closure::new(f);

        // // https://gist.github.com/kndysfm/f722e2b6dc26ab28e3da5945d5e21933

        self.hid_device
            .set_oninputreport(Some(closure.as_ref().unchecked_ref()));

        let chunks = write_apdu(&command);
        let report_id = 0x00;
        for mut chunk in chunks.into_iter() {
            console_log!("data -> {:?}", &chunk[..]);

            let promise = self
                .hid_device
                .send_report_with_u8_slice(report_id, &mut chunk[..])
                .unwrap();

            let result = wasm_bindgen_futures::JsFuture::from(promise)
                .await
                .map_err(Error::JsVal)?;
        }

        lwk_wollet::clients::asyncr::async_sleep(3500).await; // TODO: how to wait for the response?

        let result = closure_result.take();
        console_log!("apdu <- {:?}", result);

        let result = read_multi_apdu(result).unwrap();
        console_log!("result  <- {:?}", result);

        // let answer = APDUAnswer::from_answer(result).map_err(|_| "Invalid Answer")?;
        let answer = APDUAnswer::from_answer(result).unwrap();

        console_log!("answer <- {:?}", answer);
        let status = StatusWord::try_from(answer.retcode()).unwrap_or(StatusWord::Unknown);
        let vec = answer.data().to_vec();
        console_log!("status code: {:?} answer vec <- {:?}", status, vec);

        Ok((status, vec))
    }
}

//https://rustwasm.github.io/wasm-bindgen/api/web_sys/struct.HidDevice.html#method.send_report_with_u8_slice
#[wasm_bindgen(js_name = searchLedgerDevice)]
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

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = console)]
    fn log(s: &str);

    #[wasm_bindgen(js_namespace = console, js_name = log)]
    fn log_u32(a: u32);

    // Multiple arguments too!
    #[wasm_bindgen(js_namespace = console, js_name = log)]
    fn log_many(a: &str, b: &str);
}
