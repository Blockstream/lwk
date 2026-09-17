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
