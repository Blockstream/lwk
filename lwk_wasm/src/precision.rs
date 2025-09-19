use wasm_bindgen::prelude::*;

use crate::Error;

/// Helper to convert satoshi values of an asset to the value with the given precision and viceversa.
///
/// For example 100 satoshi with precision 2 is "1.00"
#[wasm_bindgen]
#[derive(Debug)]
pub struct Precision {
    inner: lwk_common::precision::Precision,
}

#[wasm_bindgen]
impl Precision {
    /// Create a new Precision, useful to encode e decode values for assets with precision.
    /// erroring if the given precision is greater than the allowed maximum (8)
    #[wasm_bindgen(constructor)]
    pub fn new(precision: u8) -> Result<Precision, Error> {
        Ok(Precision {
            inner: lwk_common::precision::Precision::new(precision)?,
        })
    }

    /// Convert the given satoshi value to the formatted value according to our precision
    ///
    /// For example 100 satoshi with precision 2 is "1.00"
    #[wasm_bindgen(js_name = satsToString)]
    pub fn sats_to_string(&self, sats: i64) -> String {
        self.inner.sats_to_string(sats)
    }

    /// Convert the given string with precision to satoshi units.
    ///
    /// For example the string "1.00" of an asset with precision 2 is 100 satoshi.
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
