use crate::LwkError;

use std::sync::Arc;

/// Transaction input sequence number.
#[derive(uniffi::Object, PartialEq, Eq, Debug, Clone, Copy)]
pub struct TxSequence {
    inner: elements::Sequence,
}

impl From<elements::Sequence> for TxSequence {
    fn from(inner: elements::Sequence) -> Self {
        TxSequence { inner }
    }
}

impl From<TxSequence> for elements::Sequence {
    fn from(value: TxSequence) -> Self {
        value.inner
    }
}

impl From<&TxSequence> for elements::Sequence {
    fn from(value: &TxSequence) -> Self {
        value.inner
    }
}

impl AsRef<elements::Sequence> for TxSequence {
    fn as_ref(&self) -> &elements::Sequence {
        &self.inner
    }
}

#[uniffi::export]
impl TxSequence {
    /// Create a sequence from a u32 value.
    #[uniffi::constructor]
    pub fn from_consensus(value: u32) -> Arc<Self> {
        Arc::new(TxSequence {
            inner: elements::Sequence::from_consensus(value),
        })
    }

    /// Zero value sequence.
    ///
    /// This sequence number enables replace-by-fee and lock-time.
    #[uniffi::constructor]
    pub fn zero() -> Arc<Self> {
        Arc::new(TxSequence {
            inner: elements::Sequence::ZERO,
        })
    }

    /// The maximum allowable sequence number.
    ///
    /// This sequence number disables lock-time and replace-by-fee.
    #[uniffi::constructor]
    pub fn max() -> Arc<Self> {
        Arc::new(TxSequence {
            inner: elements::Sequence::MAX,
        })
    }

    /// The sequence number that enables replace-by-fee and absolute lock-time but
    /// disables relative lock-time.
    #[uniffi::constructor]
    pub fn enable_rbf_no_locktime() -> Arc<Self> {
        Arc::new(TxSequence {
            inner: elements::Sequence::ENABLE_RBF_NO_LOCKTIME,
        })
    }

    /// The sequence number that enables absolute lock-time but disables replace-by-fee
    /// and relative lock-time.
    #[uniffi::constructor]
    pub fn enable_locktime_no_rbf() -> Arc<Self> {
        Arc::new(TxSequence {
            inner: elements::Sequence::ENABLE_LOCKTIME_NO_RBF,
        })
    }

    /// Create a relative lock-time using block height.
    #[uniffi::constructor]
    pub fn from_height(height: u16) -> Arc<Self> {
        Arc::new(TxSequence {
            inner: elements::Sequence::from_height(height),
        })
    }

    /// Create a relative lock-time using time intervals where each interval is equivalent
    /// to 512 seconds.
    #[uniffi::constructor]
    pub fn from_512_second_intervals(intervals: u16) -> Arc<Self> {
        Arc::new(TxSequence {
            inner: elements::Sequence::from_512_second_intervals(intervals),
        })
    }

    /// Create a relative lock-time from seconds, converting the seconds into 512 second
    /// interval with floor division.
    #[uniffi::constructor]
    pub fn from_seconds_floor(seconds: u32) -> Result<Arc<Self>, LwkError> {
        let inner =
            elements::Sequence::from_seconds_floor(seconds).map_err(|e| LwkError::Generic {
                msg: format!("TxSequence from_seconds_floor error: {e}"),
            })?;
        Ok(Arc::new(TxSequence { inner }))
    }

    /// Create a relative lock-time from seconds, converting the seconds into 512 second
    /// interval with ceiling division.
    #[uniffi::constructor]
    pub fn from_seconds_ceil(seconds: u32) -> Result<Arc<Self>, LwkError> {
        let inner =
            elements::Sequence::from_seconds_ceil(seconds).map_err(|e| LwkError::Generic {
                msg: format!("TxSequence from_seconds_ceil error: {e}"),
            })?;
        Ok(Arc::new(TxSequence { inner }))
    }

    /// Returns the inner 32bit integer value of Sequence.
    pub fn to_consensus_u32(&self) -> u32 {
        self.inner.to_consensus_u32()
    }

    /// Returns `true` if the sequence number indicates that the transaction is finalised.
    pub fn is_final(&self) -> bool {
        self.inner.is_final()
    }

    /// Returns true if the transaction opted-in to BIP125 replace-by-fee.
    pub fn is_rbf(&self) -> bool {
        self.inner.is_rbf()
    }

    /// Returns `true` if the sequence has a relative lock-time.
    pub fn is_relative_lock_time(&self) -> bool {
        self.inner.is_relative_lock_time()
    }

    /// Returns `true` if the sequence number encodes a block based relative lock-time.
    pub fn is_height_locked(&self) -> bool {
        self.inner.is_height_locked()
    }

    /// Returns `true` if the sequene number encodes a time interval based relative lock-time.
    pub fn is_time_locked(&self) -> bool {
        self.inner.is_time_locked()
    }

    /// Returns `true` if the sequence number enables absolute lock-time.
    pub fn enables_absolute_lock_time(&self) -> bool {
        self.inner.enables_absolute_lock_time()
    }
}
