use crate::Error;
use lwk_ledger::asyncr::Ledger;
use lwk_ledger::asyncr::LiquidClient;
use lwk_ledger::APDUAnswer;
use lwk_ledger::{APDUCmdVec, StatusWord};
use lwk_wollet::{bitcoin::bip32::DerivationPath, elements::pset::PartiallySignedTransaction};
use serde::Serialize;
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

#[wasm_bindgen]
struct Path {
    derivation_path: DerivationPath,
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

    //pub async fn derive_xpub(&self, path: Path) -> Result<String, Error> { //std::result::Result<Xpub, Self::Error> {
    pub async fn derive_xpub(&self) -> Result<String, Error> {
        //let derivation_path = DerivationPath::master();
        let derivation_path = DerivationPath::from_str("m/44'/1'/0'").unwrap();
        let r = self
            .ledger
            .client
            .get_extended_pubkey(&derivation_path, false)
            .await
            .map_err(|e| Error::Generic(format!("{:?} error getting XPUB derivation path {:?}.", e, derivation_path)))?;
        console_log!("r {}", r.to_string());
        Ok(r.to_string())
    }

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

    pub async  fn fingerprint(&self) -> Result<String, Error> {
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
            *c = ba;
        };
        let closure: Closure<dyn FnMut(_)> = Closure::once(f);

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

        lwk_wollet::clients::asyncr::async_sleep(500).await; // TODO: how to wait for the response?

        let result = closure_result.take();
        console_log!("apdu <- {:?}", result);

        let result = read_apdu(result);
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

const LEDGER_PACKET_WRITE_SIZE: u8 = 64;
const LEDGER_CHANNEL: u16 = 0x0101;

// based on https://github.com/Zondax/ledger-rs/blob/master/ledger-transport-hid/src/lib.rs
// with the notable difference we don't use the prefix 0x00
fn write_apdu(apdu_command: &APDUCmdVec) -> Vec<[u8; LEDGER_PACKET_WRITE_SIZE as usize]> {
    let channel = LEDGER_CHANNEL;
    let apdu_command = apdu_command.serialize();
    let mut results = vec![];
    let command_length = apdu_command.len();
    let mut in_data = Vec::with_capacity(command_length + 2);
    in_data.push(((command_length >> 8) & 0xFF) as u8);
    in_data.push((command_length & 0xFF) as u8);
    in_data.extend_from_slice(&apdu_command);

    let mut buffer = vec![0u8; LEDGER_PACKET_WRITE_SIZE as usize];
    buffer[0] = ((channel >> 8) & 0xFF) as u8; // channel big endian
    buffer[1] = (channel & 0xFF) as u8; // channel big endian
    buffer[2] = 0x05u8;

    for (sequence_idx, chunk) in in_data
        .chunks((LEDGER_PACKET_WRITE_SIZE - 5) as usize)
        .enumerate()
    {
        buffer[3] = ((sequence_idx >> 8) & 0xFF) as u8; // sequence_idx big endian
        buffer[4] = (sequence_idx & 0xFF) as u8; // sequence_idx big endian
        buffer[5..5 + chunk.len()].copy_from_slice(chunk);

        println!("[{:3}] << {:?}", buffer.len(), &buffer);

        results.push(buffer.clone().try_into().unwrap());
    }
    results
}

// fn read_apdu(apdu_answer: Vec<u8>) -> Vec<u8> {
//     let mut buffer = vec![0u8; LEDGER_PACKET_READ_SIZE as usize];
//     let mut sequence_idx = 0u16;
//     let mut expected_apdu_len = 0usize;

//     loop {
//         let res = device.read_timeout(&mut buffer, LEDGER_TIMEOUT)?;

//         if (sequence_idx == 0 && res < 7) || res < 5 {
//             return Err(LedgerHIDError::Comm("Read error. Incomplete header"));
//         }

//         let mut rdr = Cursor::new(&buffer);

//         let rcv_channel = rdr.read_u16::<BigEndian>()?;
//         let rcv_tag = rdr.read_u8()?;
//         let rcv_seq_idx = rdr.read_u16::<BigEndian>()?;

//         if rcv_channel != channel {
//             return Err(LedgerHIDError::Comm("Invalid channel"));
//         }
//         if rcv_tag != 0x05u8 {
//             return Err(LedgerHIDError::Comm("Invalid tag"));
//         }

//         if rcv_seq_idx != sequence_idx {
//             return Err(LedgerHIDError::Comm("Invalid sequence idx"));
//         }

//         if rcv_seq_idx == 0 {
//             expected_apdu_len = rdr.read_u16::<BigEndian>()? as usize;
//         }

//         let available: usize = buffer.len() - rdr.position() as usize;
//         let missing: usize = expected_apdu_len - apdu_answer.len();
//         let end_p = rdr.position() as usize + std::cmp::min(available, missing);

//         let new_chunk = &buffer[rdr.position() as usize..end_p];

//         info!("[{:3}] << {:}", new_chunk.len(), hex::encode(new_chunk));

//         apdu_answer.extend_from_slice(new_chunk);

//         if apdu_answer.len() >= expected_apdu_len {
//             return Ok(apdu_answer.len());
//         }

//         sequence_idx += 1;
//     }
// }

fn read_apdu(apdu_answer: Vec<u8>) -> Vec<u8> {
    let len = apdu_answer[6] as usize;
    let start = 7usize;
    let end = start + len;
    apdu_answer[start..end].to_vec()
}

#[cfg(all(test, target_arch = "wasm32"))]
mod tests {

    use super::write_apdu;
    use super::LEDGER_CHANNEL;
    use wasm_bindgen_test::*;

    wasm_bindgen_test_configure!(run_in_browser);

    #[wasm_bindgen_test]
    async fn test_write_apdu() {
        let command = lwk_ledger::command::get_version();
        assert_eq!(&command.serialize(), &[176u8, 1, 0, 0, 0]);
        let results = write_apdu(&command);
        assert_eq!(
            results,
            vec![[
                1, 1, 5, 0, 0, 0, 5, 176, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                0, 0, 0, 0, 0, 0, 0, 0, 0
            ]]
        );
    }

    #[wasm_bindgen_test]
    async fn test_read_apdu() {
        use lwk_wollet::elements::hex::FromHex;

        let get_version_test_vector_hex = "010e4c6971756964205265677465737405322e322e330102";
        let get_version_test_vector_bytes =
            Vec::<u8>::from_hex(get_version_test_vector_hex).unwrap();
        let get_version_test_vector_array = [
            1, 14, 76, 105, 113, 117, 105, 100, 32, 82, 101, 103, 116, 101, 115, 116, 5, 50, 46,
            50, 46, 51, 1, 2,
        ];
        assert_eq!(get_version_test_vector_bytes, get_version_test_vector_array);

        let received_apdu_ledger = [
            1, 1, 5, 0, 0, 0, 26, 1, 14, 76, 105, 113, 117, 105, 100, 32, 82, 101, 103, 116, 101,
            115, 116, 5, 50, 46, 50, 46, 51, 1, 2, 144, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        ];
        let start = 7usize;
        let len = received_apdu_ledger[6] as usize;
        let len_excluded_status_code = len - 2;
        let end = start + len_excluded_status_code;
        let received_apdu = &received_apdu_ledger[start..end];
        assert_eq!(
            received_apdu.to_vec(),
            get_version_test_vector_bytes.to_vec()
        );
    }
}
