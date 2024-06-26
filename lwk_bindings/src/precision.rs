use std::sync::Arc;

use crate::LwkError;

/// Wrapper over [`lwk_common::Precision`]
#[derive(uniffi::Object, Debug)]
pub struct Precision {
    inner: lwk_common::Precision,
}

#[uniffi::export]
impl Precision {
    #[uniffi::constructor]

    /// See [`lwk_common::Precision::new`]
    pub fn new(precision: u8) -> Result<Arc<Precision>, LwkError> {
        Ok(Arc::new(Precision {
            inner: lwk_common::Precision::new(precision)?,
        }))
    }

    /// See [`lwk_common::Precision::sats_to_string`]
    pub fn sats_to_string(&self, sats: i64) -> String {
        self.inner.sats_to_string(sats)
    }

    /// See [`lwk_common::Precision::string_to_sats`]
    pub fn string_to_sats(&self, val: &str) -> Result<i64, LwkError> {
        Ok(self.inner.string_to_sats(val)?)
    }
}

#[cfg(test)]
mod tests {
    use crate::Precision;

    #[test]
    fn test_precision() {
        let precision = Precision::new(2).unwrap();
        assert_eq!(precision.sats_to_string(100), "1.00");
        assert_eq!(precision.string_to_sats("1").unwrap(), 100);
    }
}
