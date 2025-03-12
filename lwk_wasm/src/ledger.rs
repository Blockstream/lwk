use crate::Error;
use byteorder::BigEndian;
use byteorder::ReadBytesExt;
use lwk_ledger::asyncr::Ledger;
use lwk_ledger::asyncr::LiquidClient;
use lwk_ledger::parse_multisig;
use lwk_ledger::read_multi_apdu;
use lwk_ledger::write_apdu;
use lwk_ledger::APDUAnswer;
use lwk_ledger::AddressType;
use lwk_ledger::PartialSignature;
use lwk_ledger::Version;
use lwk_ledger::WalletPolicy;
use lwk_ledger::WalletPubKey;
use lwk_ledger::{APDUCmdVec, StatusWord};
use lwk_wollet::elements_miniscript;
use lwk_wollet::{
    bitcoin::bip32::ChildNumber, bitcoin::bip32::DerivationPath, bitcoin::bip32::Fingerprint,
    elements::pset::PartiallySignedTransaction,
};
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

    #[wasm_bindgen(js_name = sign)]
    pub async fn sign(&self, pset_str: &str) -> Result<String, Error> {
        let mut pset: PartiallySignedTransaction =
            PartiallySignedTransaction::from_str(pset_str).expect("PSET parsing");
        // Set the default values some fields that Ledger requires
        if pset.global.tx_data.fallback_locktime.is_none() {
            pset.global.tx_data.fallback_locktime =
                Some(elements_miniscript::elements::LockTime::ZERO);
        }
        for input in pset.inputs_mut() {
            if input.sequence.is_none() {
                input.sequence = Some(elements_miniscript::elements::Sequence::default());
            }
        }

        // Use a map to avoid inserting a wallet twice
        let mut wallets = std::collections::HashMap::<String, WalletPolicy>::new();
        let mut n_sigs = 0;
        let master_fp = self.fingerprint().await?;

        // VALE: self return a string, we need to convert it
        let master_fp_obj = Fingerprint::from_str(&master_fp)?;

        // Figure out which wallets are signing
        'outer: for input in pset.inputs() {
            let is_p2wpkh = input
                .witness_utxo
                .as_ref()
                .map(|u| u.script_pubkey.is_v0_p2wpkh())
                .unwrap_or(false);
            let is_p2sh = input
                .witness_utxo
                .as_ref()
                .map(|u| u.script_pubkey.is_p2sh())
                .unwrap_or(false);
            let is_p2shwpkh = is_p2sh
                && input
                    .redeem_script
                    .as_ref()
                    .map(|x| x.is_v0_p2wpkh())
                    .unwrap_or(false);
            // Singlesig
            if is_p2wpkh || is_p2shwpkh {
                // We expect exactly one element
                if let Some((fp, path)) = input.bip32_derivation.values().next() {
                    if fp == &master_fp_obj {
                        // TODO: check path
                        // path has len 3
                        // path has all hardened
                        // path has purpose matching address type
                        // path has correct coin type
                        let mut v: Vec<ChildNumber> = path.clone().into();
                        v.truncate(3);
                        let path: DerivationPath = v.into();

                        // Do we care about the descriptor blinding key here?
                        let name = "".to_string();
                        let version = Version::V2;
                        // TODO: cache xpubs
                        let xpub = self
                            .ledger
                            .client
                            .get_extended_pubkey(&path, false)
                            .await
                            .map_err(|e| Error::Generic(format!("{:?} error getting xpub", e)))?;
                        let key = WalletPubKey::from(((*fp, path.clone()), xpub));
                        let keys = vec![key];
                        let desc = if is_p2wpkh {
                            "wpkh(@0/**)"
                        } else {
                            "sh(wpkh(@0/**))"
                        };
                        let wallet_policy =
                            WalletPolicy::new(name, version, desc.to_string(), keys);
                        let is_change = false;
                        if let Ok(d) = wallet_policy.get_descriptor(is_change) {
                            wallets.insert(d, wallet_policy);
                        }
                    }
                }
            } else {
                let is_p2wsh = input
                    .witness_utxo
                    .as_ref()
                    .map(|u| u.script_pubkey.is_v0_p2wsh())
                    .unwrap_or(false);
                let details = input.witness_script.as_ref().and_then(parse_multisig);
                // Multisig
                if is_p2wsh {
                    if let Some((threshold, pubkeys)) = details {
                        let mut keys: Vec<WalletPubKey> = vec![];
                        for pubkey in pubkeys {
                            if let Some((fp, path)) = input.bip32_derivation.get(&pubkey) {
                                let mut v: Vec<ChildNumber> = path.clone().into();
                                v.truncate(3);
                                let path: DerivationPath = v.into();
                                let keysource = (*fp, path);
                                if let Some(xpub) = pset.global.xpub.iter().find_map(|(x, ks)| {
                                    if ks == &keysource {
                                        Some(x)
                                    } else {
                                        None
                                    }
                                }) {
                                    let mut key = WalletPubKey::from((keysource, *xpub));
                                    key.multipath = Some("/**".to_string());
                                    keys.push(key);
                                } else {
                                    // Global xpub not available, cannot reconstruct the script
                                    continue 'outer;
                                }
                            } else {
                                // No keysource for pubkey in script
                                // Either the script is not ours or data is missing
                                continue 'outer;
                            }
                        }
                        let sorted = false;
                        let wallet_policy = WalletPolicy::new_multisig(
                            "todo".to_string(),
                            Version::V1,
                            AddressType::NativeSegwit,
                            threshold as usize,
                            keys,
                            sorted,
                            None,
                        )
                        .expect("FIXME");
                        let is_change = false;
                        if let Ok(d) = wallet_policy.get_descriptor(is_change) {
                            wallets.insert(d, wallet_policy);
                        }
                    }
                }
            }
        }

        // For each wallet, sign
        for wallet_policy in wallets.values() {
            let hmac = if wallet_policy.threshold.is_some() {
                // Register multisig wallets
                let (_id, hmac) = self
                    .ledger
                    .client
                    .register_wallet(wallet_policy)
                    .await
                    .map_err(|e| Error::Generic(format!("{:?} error getting xpub", e)))?;
                Some(hmac)
            } else {
                None
            };
            let partial_sigs = self
                .ledger
                .client
                .sign_psbt(&pset, wallet_policy, hmac.as_ref())
                .await
                .map_err(|e| Error::Generic(format!("{:?} error getting sign", e)))?;
            n_sigs += partial_sigs.len();

            // Add sigs to pset
            for (input_idx, sig) in partial_sigs {
                let input = &mut pset.inputs_mut()[input_idx];
                for (public_key, (fp, _origin)) in &input.bip32_derivation {
                    if fp == &master_fp_obj {
                        // TODO: user the pubkey from PartialSignature to insert in partial_sigs
                        let sig_vec = match sig {
                            PartialSignature::Sig(_, sig) => sig.to_vec(),
                            _ => panic!("FIXME: support taproot sig or raise error"),
                        };
                        input.partial_sigs.insert(*public_key, sig_vec);
                        // FIXME: handle cases where we have multiple pubkeys with master fingerprint
                        break;
                    }
                }
            }
        }

        console_log!("signed pset {}", pset.to_string());
        Ok(pset.to_string())
    }
}

impl lwk_ledger::asyncr::Transport for TransportWeb {
    type Error = Error;

    async fn exchange(&self, command: &APDUCmdVec) -> Result<(StatusWord, Vec<u8>), Self::Error> {
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

        // https://gist.github.com/kndysfm/f722e2b6dc26ab28e3da5945d5e21933

        self.hid_device
            .set_oninputreport(Some(closure.as_ref().unchecked_ref()));

        let chunks = write_apdu(&command);
        let report_id = 0x00;
        for mut chunk in chunks.into_iter() {
            // console_log!("data -> {:?}", &chunk[..]);

            let promise = self
                .hid_device
                .send_report_with_u8_slice(report_id, &mut chunk[..])
                .unwrap();

            let result = wasm_bindgen_futures::JsFuture::from(promise)
                .await
                .map_err(Error::JsVal)?;
        }

        let sleep_ms = 100;
        let mut attempts = (1000 / sleep_ms) * 10 * 60; // 10 minutes
        loop {
            lwk_wollet::clients::asyncr::async_sleep(sleep_ms).await;
            if !closure_result.borrow().is_empty() {
                let copy = closure_result.borrow().clone();
                if let Ok(_) = read_multi_apdu(copy) {
                    break;
                }
            }
            attempts -= 1;
            if attempts == 0 {
                console_log!("Timeout waiting for response");
                return Err(Error::Generic("Timeout waiting for response".to_string()));
            }
        }

        let result = closure_result.take();

        let result = read_multi_apdu(result).unwrap();

        let answer = APDUAnswer::from_answer(result).unwrap();

        let status = StatusWord::try_from(answer.retcode()).unwrap_or(StatusWord::Unknown);
        let vec = answer.data().to_vec();
        console_log!("status code: {:?} ", status);

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
        let result = wasm_bindgen_futures::JsFuture::from(promise)
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
