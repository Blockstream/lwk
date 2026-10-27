//! `async_sleep` must work where there is no browser window, as in Web Workers.
//!
//! Scans race against `async_sleep` for their deadline, so if it fails there every scan fails.
#![cfg(target_arch = "wasm32")]

use wasm_bindgen_futures::js_sys::Date;
use wasm_bindgen_test::*;

wasm_bindgen_test_configure!(run_in_dedicated_worker);

#[wasm_bindgen_test]
async fn async_sleep_works_in_worker() {
    let start = Date::now();
    lwk_wollet::asyncr::async_sleep(50).await.unwrap();
    assert!(Date::now() - start >= 49.0);
}
