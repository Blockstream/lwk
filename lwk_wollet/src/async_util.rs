// based on https://users.rust-lang.org/t/rust-wasm-async-sleeping-for-100-milli-seconds-goes-up-to-1-minute/81177
// TODO remove/handle/justify unwraps
/// Sleep asynchronously for the given number of milliseconds on WASM targets.
#[cfg(target_arch = "wasm32")]
pub async fn async_sleep(millis: u64) {
    let mut cb = |resolve: js_sys::Function, _reject: js_sys::Function| {
        web_sys::window()
            .unwrap()
            .set_timeout_with_callback_and_timeout_and_arguments_0(&resolve, millis as i32)
            .unwrap();
    };
    let p = js_sys::Promise::new(&mut cb);
    wasm_bindgen_futures::JsFuture::from(p).await.unwrap();
}

#[cfg(not(target_arch = "wasm32"))]
/// Sleep asynchronously for the given number of milliseconds on non-WASM targets.
pub async fn async_sleep(millis: u64) {
    tokio::time::sleep(tokio::time::Duration::from_millis(millis)).await;
}

#[cfg(not(target_arch = "wasm32"))]
/// Get the current time in milliseconds since the UNIX epoch
pub async fn async_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("Failed to get current time")
        .as_millis() as u64
}

#[cfg(target_arch = "wasm32")]
/// Get the current time in milliseconds since the UNIX epoch
pub async fn async_now() -> u64 {
    js_sys::Date::now() as u64
}
