use crate::{LwkError, Txid};
use std::{fmt::Display, sync::Arc};

/// A reference to a transaction output
#[derive(uniffi::Object)]
#[uniffi::export(Display)]
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

impl From<&OutPoint> for elements::OutPoint {
    fn from(o: &OutPoint) -> Self {
        o.inner
    }
}

impl Display for OutPoint {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.inner)
    }
}

#[uniffi::export]
impl OutPoint {
    /// Construct an OutPoint object from its string representation.
    /// For example: "[elements]0000000000000000000000000000000000000000000000000000000000000001:1"
    /// To create the string representation of an outpoint use `to_string()`.
    #[uniffi::constructor]
    pub fn new(s: &str) -> Result<Arc<Self>, LwkError> {
        let inner: elements::OutPoint = s.parse()?;
        Ok(Arc::new(Self { inner }))
    }

    /// Return the transaction identifier.
    pub fn txid(&self) -> Arc<Txid> {
        Arc::new(self.inner.txid.into())
    }

    /// Return the output index.
    pub fn vout(&self) -> u32 {
        self.inner.vout
    }
}

#[cfg(test)]
mod tests {
    use crate::OutPoint;
    use std::str::FromStr;

    #[test]
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
