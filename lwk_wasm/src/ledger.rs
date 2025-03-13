use crate::Error;
use lwk_ledger::asyncr::Ledger;
use lwk_ledger::read_multi_apdu;
use lwk_ledger::write_apdu;
use lwk_ledger::APDUAnswer;
use lwk_ledger::{APDUCmdVec, StatusWord};
use lwk_wollet::{bitcoin::bip32::DerivationPath, elements::pset::PartiallySignedTransaction};
use serde::Serialize;
use std::cell::RefCell;
use std::rc::Rc;
use std::str::FromStr;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::HidDevice;
use web_sys::HidDeviceRequestOptions;
use web_sys::Performance;

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

    closure: Closure<dyn FnMut(web_sys::HidInputReportEvent)>,
    closure_result: Rc<RefCell<Vec<Vec<u8>>>>,
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
        let closure_result = std::rc::Rc::new(std::cell::RefCell::new(vec![]));

        let result_clone = closure_result.clone();

        let f = move |e: web_sys::HidInputReportEvent| {
            let dataview = e.data();

            let ofs = dataview.byte_offset();
            let len = dataview.byte_length();

            let ba: Vec<u8> = (0..len).map(|i| dataview.get_uint8(i + ofs)).collect();

            let mut c = result_clone.borrow_mut();
            (*c).push(ba);
        };
        let closure: Closure<dyn FnMut(_)> = Closure::new(f);

        let transport = TransportWeb {
            hid_device,
            closure,
            closure_result,
        };

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

    /// TODO Should use Signer::wpkh_slip77_descriptor
    #[wasm_bindgen(js_name = wpkhSlip77Descriptor)]
    pub async fn wpkh_slip77_descriptor(&self) -> Result<String, Error> {
        let blinding = self.slip77_master_blinding_key().await?;
        let fingerprint = self.fingerprint().await?;
        let path = "84'/1'/0'";
        let xpub = self.derive_xpub(path).await?;
        let is_mainnet = false; // TODO handle mainnet
        let script_variant = lwk_common::Singlesig::Wpkh;
        let blinding_variant = lwk_common::DescriptorBlindingKey::Slip77;

        Ok(format!(
            "ct(slip77({blinding}),elwpkh([{fingerprint}/{path}]{xpub}/<0;1>/*))"
        ))
    }

    pub async fn sign(&self, pset_str: &str) -> Result<String, Error> {
        let performance = get_performance()?;
        let start_time = performance.now();

        let mut pset = PartiallySignedTransaction::from_str(&pset_str)
            .map_err(|e| Error::Generic(format!("{:?} error parsing pset", e)))?;
        let _ = self
            .ledger
            .client
            .sign(&mut pset)
            .await
            .map_err(|e| Error::Generic(format!("{:?} error signing", e)))?;

        console_log!(
            "Time taken for signing: {:?}ms",
            performance.now() - start_time
        );
        Ok(pset.to_string())
    }
}

impl lwk_ledger::asyncr::Transport for TransportWeb {
    type Error = Error;

    async fn exchange(&self, command: &APDUCmdVec) -> Result<(StatusWord, Vec<u8>), Self::Error> {
        #[cfg(debug_assertions)]
        let performance = get_performance()?;
        #[cfg(debug_assertions)]
        let start_time = performance.now();

        // https://gist.github.com/kndysfm/f722e2b6dc26ab28e3da5945d5e21933
        self.closure_result.borrow_mut().clear();
        self.hid_device
            .set_oninputreport(Some(self.closure.as_ref().unchecked_ref()));

        let chunks = write_apdu(&command);
        let report_id = 0x00;
        for mut chunk in chunks.into_iter() {
            // console_log!("data -> {:?}", &chunk[..]);

            let promise = self
                .hid_device
                .send_report_with_u8_slice(report_id, &mut chunk[..])
                .unwrap();

            let _result = wasm_bindgen_futures::JsFuture::from(promise)
                .await
                .map_err(Error::JsVal)?;
        }

        let mut attempts = 0;
        let sleep_ms = 10;
        let total_attempts = (1000 / sleep_ms) * 10 * 60; // 10 minutes
        let result = loop {
            if !self.closure_result.borrow().is_empty() {
                let copy = self.closure_result.borrow().clone();
                if let Ok(result) = read_multi_apdu(copy) {
                    self.closure_result.borrow_mut().clear();
                    break result;
                }
            }
            attempts += 1;
            lwk_wollet::clients::asyncr::async_sleep(sleep_ms).await;

            if attempts >= total_attempts {
                console_log!("Timeout waiting for response");
                return Err(Error::Generic("Timeout waiting for response".to_string()));
            }
        };

        let answer = APDUAnswer::from_answer(result).unwrap();

        let status = StatusWord::try_from(answer.retcode()).unwrap_or(StatusWord::Unknown);
        let vec = answer.data().to_vec();

        #[cfg(debug_assertions)]
        console_log!(
            "status code: {:?} time: {:?}ms attempts: {}",
            status,
            performance.now() - start_time,
            attempts
        );

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

        if hids.length() > 0 {
            // TODO handle multiple ledgers?
            js_val_to_hid_device(hids.get(0))?
        } else {
            return Err(Error::Generic("no ledger found".to_string()));
        }
    };

    if !hid_device.opened() {
        let promise = hid_device.open();
        let _result = wasm_bindgen_futures::JsFuture::from(promise)
            .await
            .map_err(Error::JsVal)?;
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

fn get_performance() -> Result<Performance, Error> {
    let window =
        web_sys::window().ok_or_else(|| Error::Generic("cannot get window".to_string()))?;
    window
        .performance()
        .ok_or_else(|| Error::Generic("cannot get performance".to_string()))
}
