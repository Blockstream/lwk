// based on https://users.rust-lang.org/t/rust-wasm-async-sleeping-for-100-milli-seconds-goes-up-to-1-minute/81177
use crate::Error;

/// Sleep asynchronously for the given number of milliseconds on WASM targets.
///
/// Uses the global `setTimeout`, so it works in browser windows, Web Workers and Node.js.
/// If the returned future is dropped before the timer fires, the timer is cleared: a pending
/// timer would otherwise keep a Node.js process alive until it fires.
#[cfg(target_arch = "wasm32")]
pub async fn async_sleep(millis: u64) -> Result<(), Error> {
    let global = js_sys::global();
    let mut timer = None;
    let mut cb = |resolve: js_sys::Function, reject: js_sys::Function| {
        let result = global_function(&global, "setTimeout")
            .ok_or(Error::AsyncSleepMissingWindow)
            .and_then(|set_timeout| {
                let millis = js_sys::Number::from(millis.min(i32::MAX as u64) as f64);
                set_timeout
                    .call2(&global, &resolve, &millis)
                    .map_err(|err| Error::AsyncSleepFailed(format!("{err:?}")))
            });

        match result {
            Ok(id) => {
                timer = Some(ClearTimeoutOnDrop {
                    global: global.clone(),
                    id,
                })
            }
            Err(err) => {
                let _ = reject.call1(&js_sys::Object::new().into(), &err.to_string().into());
            }
        }
    };
    let p = js_sys::Promise::new(&mut cb);
    let _timer = timer;
    wasm_bindgen_futures::JsFuture::from(p)
        .await
        .map_err(|err| Error::AsyncSleepFailed(format!("{err:?}")))?;
    Ok(())
}

/// The function named `name` on the global object, if any.
#[cfg(target_arch = "wasm32")]
fn global_function(global: &js_sys::Object, name: &str) -> Option<js_sys::Function> {
    js_sys::Reflect::get(global, &js_sys::JsString::from(name))
        .ok()
        .filter(|value| value.is_function())
        .map(js_sys::Function::from)
}

/// Clears a `setTimeout` timer when dropped, a no-op if it has already fired.
#[cfg(target_arch = "wasm32")]
struct ClearTimeoutOnDrop {
    global: js_sys::Object,
    id: js_sys::wasm_bindgen::JsValue,
}

#[cfg(target_arch = "wasm32")]
impl Drop for ClearTimeoutOnDrop {
    fn drop(&mut self) {
        if let Some(clear_timeout) = global_function(&self.global, "clearTimeout") {
            let _ = clear_timeout.call1(&self.global, &self.id);
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
/// Sleep asynchronously for the given number of milliseconds on non-WASM targets.
pub async fn async_sleep(millis: u64) -> Result<(), Error> {
    tokio::time::sleep(tokio::time::Duration::from_millis(millis)).await;
    Ok(())
}

/// Run `future`, failing with the error built by `on_timeout` if it does not complete within
/// `timeout`.
#[cfg(feature = "esplora")]
pub(crate) async fn with_timeout<T>(
    timeout: std::time::Duration,
    future: impl std::future::Future<Output = Result<T, Error>>,
    on_timeout: impl FnOnce() -> Error,
) -> Result<T, Error> {
    let millis = u64::try_from(timeout.as_millis()).unwrap_or(u64::MAX);
    tokio::select! {
        result = future => result,
        sleep_result = async_sleep(millis) => {
            sleep_result?;
            Err(on_timeout())
        }
    }
}

#[cfg(all(test, feature = "esplora", not(target_arch = "wasm32")))]
mod tests {
    use std::time::Duration;

    use super::with_timeout;
    use crate::Error;

    #[tokio::test]
    async fn with_timeout_fails_only_when_future_does_not_complete() {
        let timeout = Duration::from_millis(10);

        let result: Result<(), Error> = with_timeout(timeout, std::future::pending(), || {
            Error::ScanTimeout(timeout)
        })
        .await;
        assert!(matches!(result, Err(Error::ScanTimeout(t)) if t == timeout));

        let result = with_timeout(timeout, async { Ok(42) }, || Error::ScanTimeout(timeout)).await;
        assert_eq!(result.unwrap(), 42);
    }
}
