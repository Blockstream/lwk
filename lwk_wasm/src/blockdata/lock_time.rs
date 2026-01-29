use crate::Error;
use lwk_wollet::elements;
use std::fmt::Display;
use wasm_bindgen::prelude::*;

/// Transaction lock time.
///
/// See [`elements::LockTime`] for more details.
#[wasm_bindgen]
pub struct LockTime {
    inner: elements::LockTime,
}

impl From<elements::LockTime> for LockTime {
    fn from(inner: elements::LockTime) -> Self {
        Self { inner }
    }
}

impl From<LockTime> for elements::LockTime {
    fn from(value: LockTime) -> Self {
        value.inner
    }
}

impl From<&LockTime> for elements::LockTime {
    fn from(value: &LockTime) -> Self {
        value.inner
    }
}

impl AsRef<elements::LockTime> for LockTime {
    fn as_ref(&self) -> &elements::LockTime {
        &self.inner
    }
}

impl Display for LockTime {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.inner)
    }
}

#[wasm_bindgen]
impl LockTime {
    /// Create a LockTime from a consensus u32 value.
    ///
    /// See [`elements::LockTime::from_consensus`].
    #[wasm_bindgen(constructor)]
    pub fn from_consensus(value: u32) -> LockTime {
        LockTime {
            inner: elements::LockTime::from_consensus(value),
        }
    }

    /// Create a LockTime from a block height.
    ///
    /// See [`elements::LockTime::from_height`].
    pub fn from_height(height: u32) -> Result<LockTime, Error> {
        let inner = elements::LockTime::from_height(height)
            .map_err(|e| Error::Generic(format!("LockTime from_height error: {e}")))?;
        Ok(LockTime { inner })
    }

    /// Create a LockTime from a Unix timestamp.
    ///
    /// See [`elements::LockTime::from_time`].
    pub fn from_time(time: u32) -> Result<LockTime, Error> {
        let inner = elements::LockTime::from_time(time)
            .map_err(|e| Error::Generic(format!("LockTime from_time error: {e}")))?;
        Ok(LockTime { inner })
    }

    /// Create a LockTime with value zero (no lock time).
    ///
    /// See [`elements::LockTime::ZERO`].
    pub fn zero() -> LockTime {
        LockTime {
            inner: elements::LockTime::ZERO,
        }
    }

    /// Return the consensus u32 value.
    ///
    /// See [`elements::LockTime::to_consensus_u32`].
    pub fn to_consensus_u32(&self) -> u32 {
        self.inner.to_consensus_u32()
    }

    /// Return true if this lock time represents a block height.
    ///
    /// See [`elements::LockTime::is_block_height`].
    pub fn is_block_height(&self) -> bool {
        self.inner.is_block_height()
    }

    /// Return true if this lock time represents a Unix timestamp.
    ///
    /// See [`elements::LockTime::is_block_time`].
    pub fn is_block_time(&self) -> bool {
        self.inner.is_block_time()
    }

    /// Return the string representation.
    #[wasm_bindgen(js_name = toString)]
    pub fn to_string_js(&self) -> String {
        self.to_string()
    }
}

#[cfg(all(test, target_arch = "wasm32"))]
mod tests {
    use super::LockTime;
    use wasm_bindgen_test::*;

    wasm_bindgen_test_configure!(run_in_browser);

    #[wasm_bindgen_test]
    fn test_lock_time_from_consensus() {
        // Height value (< 500_000_000)
        let lt = LockTime::from_consensus(100);
        assert_eq!(lt.to_consensus_u32(), 100);
        assert!(lt.is_block_height());
        assert!(!lt.is_block_time());

        // Time value (>= 500_000_000)
        let lt = LockTime::from_consensus(500_000_001);
        assert_eq!(lt.to_consensus_u32(), 500_000_001);
        assert!(!lt.is_block_height());
        assert!(lt.is_block_time());
    }

    #[wasm_bindgen_test]
    fn test_lock_time_from_height() {
        let lt = LockTime::from_height(100).unwrap();
        assert_eq!(lt.to_consensus_u32(), 100);
        assert!(lt.is_block_height());

        // Should fail for values >= 500_000_000
        assert!(LockTime::from_height(500_000_000).is_err());
    }

    #[wasm_bindgen_test]
    fn test_lock_time_from_time() {
        let lt = LockTime::from_time(500_000_001).unwrap();
        assert_eq!(lt.to_consensus_u32(), 500_000_001);
        assert!(lt.is_block_time());

        // Should fail for values < 500_000_000
        assert!(LockTime::from_time(100).is_err());
    }

    #[wasm_bindgen_test]
    fn test_lock_time_zero() {
        let lt = LockTime::zero();
        assert_eq!(lt.to_consensus_u32(), 0);
        assert!(lt.is_block_height());
    }
}
