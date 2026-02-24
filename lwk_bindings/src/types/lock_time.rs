use crate::LwkError;

use std::fmt::Display;
use std::sync::Arc;

/// Transaction lock time.
#[derive(uniffi::Object, PartialEq, Eq, Debug, Clone, Copy)]
pub struct LockTime {
    inner: elements::LockTime,
}

impl From<elements::LockTime> for LockTime {
    fn from(inner: elements::LockTime) -> Self {
        LockTime { inner }
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

#[uniffi::export]
impl LockTime {
    /// Create a LockTime from a consensus u32 value.
    #[uniffi::constructor]
    pub fn from_consensus(value: u32) -> Arc<Self> {
        Arc::new(LockTime {
            inner: elements::LockTime::from_consensus(value),
        })
    }

    /// Create a LockTime from a block height.
    #[uniffi::constructor]
    pub fn from_height(height: u32) -> Result<Arc<Self>, LwkError> {
        let inner = elements::LockTime::from_height(height)?;
        Ok(Arc::new(LockTime { inner }))
    }

    /// Create a LockTime from a Unix timestamp.
    #[uniffi::constructor]
    pub fn from_time(time: u32) -> Result<Arc<Self>, LwkError> {
        let inner = elements::LockTime::from_time(time)?;
        Ok(Arc::new(LockTime { inner }))
    }

    /// Create a LockTime with value zero (no lock time).
    #[uniffi::constructor]
    pub fn zero() -> Arc<Self> {
        Arc::new(LockTime {
            inner: elements::LockTime::ZERO,
        })
    }

    /// Return the consensus u32 value.
    pub fn to_consensus_u32(&self) -> u32 {
        self.inner.to_consensus_u32()
    }

    /// Return true if this lock time represents a block height.
    pub fn is_block_height(&self) -> bool {
        self.inner.is_block_height()
    }

    /// Return true if this lock time represents a Unix timestamp.
    pub fn is_block_time(&self) -> bool {
        self.inner.is_block_time()
    }
}

#[cfg(test)]
mod tests {
    use super::LockTime;

    #[test]
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

    #[test]
    fn test_lock_time_from_height() {
        let lt = LockTime::from_height(100).unwrap();
        assert_eq!(lt.to_consensus_u32(), 100);
        assert!(lt.is_block_height());

        // Should fail for values >= 500_000_000
        assert!(LockTime::from_height(500_000_000).is_err());
    }

    #[test]
    fn test_lock_time_from_time() {
        let lt = LockTime::from_time(500_000_001).unwrap();
        assert_eq!(lt.to_consensus_u32(), 500_000_001);
        assert!(lt.is_block_time());

        // Should fail for values < 500_000_000
        assert!(LockTime::from_time(100).is_err());
    }

    #[test]
    fn test_lock_time_zero() {
        let lt = LockTime::zero();
        assert_eq!(lt.to_consensus_u32(), 0);
        assert!(lt.is_block_height());
    }
}
