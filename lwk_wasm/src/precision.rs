use wasm_bindgen::prelude::*;

use crate::Error;

/// Wrapper of [`lwk_common::precision::Precision`]
#[wasm_bindgen]
#[derive(Debug)]
pub struct Precision {
    inner: lwk_common::precision::Precision,
}

#[wasm_bindgen]
impl Precision {
    /// Creates a Precision
    #[wasm_bindgen(constructor)]
    pub fn new(precision: u8) -> Result<Precision, Error> {
        Ok(Precision {
            inner: lwk_common::precision::Precision::new(precision)?,
        })
    }

    #[wasm_bindgen(js_name = satsToString)]
    pub fn sats_to_string(&self, sats: i64) -> String {
        self.inner.sats_to_string(sats)
    }

    #[wasm_bindgen(js_name = stringToSats)]
    pub fn string_to_sats(&self, sats: &str) -> Result<i64, Error> {
        Ok(self.inner.string_to_sats(sats)?)
    }
}

#[cfg(all(test, target_arch = "wasm32"))]
mod tests {
    use wasm_bindgen_test::wasm_bindgen_test;

    use crate::Precision;

    #[wasm_bindgen_test]
    fn test_precision() {
        let p = Precision::new(2).unwrap();
        assert_eq!(p.sats_to_string(100), "1.00");
        assert_eq!(p.string_to_sats("1").unwrap(), 100);
    }
}
