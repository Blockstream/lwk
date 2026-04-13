// based on https://users.rust-lang.org/t/rust-wasm-async-sleeping-for-100-milli-seconds-goes-up-to-1-minute/81177
use crate::Error;

/// Sleep asynchronously for the given number of milliseconds on WASM targets.
#[cfg(target_arch = "wasm32")]
pub async fn async_sleep(millis: u64) -> Result<(), Error> {
    let mut cb = |resolve: js_sys::Function, reject: js_sys::Function| {
        let result = web_sys::window()
            .ok_or(Error::AsyncSleepMissingWindow)
            .and_then(|window| {
                window
                    .set_timeout_with_callback_and_timeout_and_arguments_0(&resolve, millis as i32)
                    .map_err(|err| Error::AsyncSleepFailed(format!("{err:?}")))
            });

        if let Err(err) = result {
            let _ = reject.call1(&js_sys::Object::new().into(), &err.to_string().into());
        }
    };
    let p = js_sys::Promise::new(&mut cb);
    wasm_bindgen_futures::JsFuture::from(p)
        .await
        .map_err(|err| Error::AsyncSleepFailed(format!("{err:?}")))?;
    Ok(())
}

#[cfg(not(target_arch = "wasm32"))]
/// Sleep asynchronously for the given number of milliseconds on non-WASM targets.
pub async fn async_sleep(millis: u64) -> Result<(), Error> {
    tokio::time::sleep(tokio::time::Duration::from_millis(millis)).await;
    Ok(())
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
