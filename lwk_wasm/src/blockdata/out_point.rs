use crate::{Error, Txid};
use lwk_wollet::elements;
use std::fmt::Display;
use wasm_bindgen::prelude::*;

/// A reference to a transaction output
#[wasm_bindgen]
pub struct OutPoint {
    inner: elements::OutPoint,
}

impl From<elements::OutPoint> for OutPoint {
    fn from(inner: elements::OutPoint) -> Self {
        Self { inner }
    }
}

impl From<OutPoint> for elements::OutPoint {
    fn from(o: OutPoint) -> Self {
        o.inner
    }
}

impl Display for OutPoint {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.inner)
    }
}

impl From<&OutPoint> for elements::OutPoint {
    fn from(o: &OutPoint) -> Self {
        o.inner
    }
}

#[wasm_bindgen]
impl OutPoint {
    /// Creates an `OutPoint` from a string representation.
    #[wasm_bindgen(constructor)]
    pub fn new(s: &str) -> Result<OutPoint, Error> {
        let out_point: elements::OutPoint = s.parse()?;
        Ok(out_point.into())
    }

    /// Creates an `OutPoint` from a transaction ID and output index.
    #[wasm_bindgen(js_name = fromParts)]
    pub fn from_parts(txid: &Txid, vout: u32) -> OutPoint {
        let inner = elements::OutPoint::new((*txid).into(), vout);
        OutPoint { inner }
    }

    /// Return the transaction identifier.
    pub fn txid(&self) -> Txid {
        self.inner.txid.into()
    }

    /// Return the output index.
    pub fn vout(&self) -> u32 {
        self.inner.vout
    }
}

#[cfg(all(test, target_arch = "wasm32"))]
mod tests {
    use crate::OutPoint;
    use lwk_wollet::elements;
    use std::str::FromStr;
    use wasm_bindgen_test::*;

    wasm_bindgen_test_configure!(run_in_browser);

    #[wasm_bindgen_test]
    fn out_point() {
        let expected_txid = "0000000000000000000000000000000000000000000000000000000000000001";
        let expected_vout = 1;
        let expected = format!("[elements]{expected_txid}:{expected_vout}");
        let out_point_elements = elements::OutPoint::new(
            elements::Txid::from_str(expected_txid).unwrap(),
            expected_vout,
        );

        assert_eq!(expected, out_point_elements.to_string());
        let out_point_bindings = OutPoint::new(&expected).unwrap();
        assert_eq!(expected, out_point_bindings.to_string());

        let out_point: OutPoint = out_point_elements.into();
        assert_eq!(expected, out_point.to_string());

        assert_eq!(expected_txid, out_point.txid().to_string());

        assert_eq!(expected_vout, out_point.vout());
    }
}
