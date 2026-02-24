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
    /// See [`elements::Sequence::from_consensus`].
    #[uniffi::constructor]
    pub fn from_consensus(value: u32) -> Arc<Self> {
        Arc::new(TxSequence {
            inner: elements::Sequence::from_consensus(value),
        })
    }

    /// See [`elements::Sequence::ZERO`].
    #[uniffi::constructor]
    pub fn zero() -> Arc<Self> {
        Arc::new(TxSequence {
            inner: elements::Sequence::ZERO,
        })
    }

    /// See [`elements::Sequence::MAX`].
    #[uniffi::constructor]
    pub fn max() -> Arc<Self> {
        Arc::new(TxSequence {
            inner: elements::Sequence::MAX,
        })
    }

    /// See [`elements::Sequence::ENABLE_RBF_NO_LOCKTIME`].
    #[uniffi::constructor]
    pub fn enable_rbf_no_locktime() -> Arc<Self> {
        Arc::new(TxSequence {
            inner: elements::Sequence::ENABLE_RBF_NO_LOCKTIME,
        })
    }

    /// See [`elements::Sequence::ENABLE_LOCKTIME_NO_RBF`].
    #[uniffi::constructor]
    pub fn enable_locktime_no_rbf() -> Arc<Self> {
        Arc::new(TxSequence {
            inner: elements::Sequence::ENABLE_LOCKTIME_NO_RBF,
        })
    }

    /// See [`elements::Sequence::from_height`].
    #[uniffi::constructor]
    pub fn from_height(height: u16) -> Arc<Self> {
        Arc::new(TxSequence {
            inner: elements::Sequence::from_height(height),
        })
    }

    /// See [`elements::Sequence::from_512_second_intervals`].
    #[uniffi::constructor]
    pub fn from_512_second_intervals(intervals: u16) -> Arc<Self> {
        Arc::new(TxSequence {
            inner: elements::Sequence::from_512_second_intervals(intervals),
        })
    }

    /// See [`elements::Sequence::from_seconds_floor`].
    #[uniffi::constructor]
    pub fn from_seconds_floor(seconds: u32) -> Result<Arc<Self>, LwkError> {
        let inner =
            elements::Sequence::from_seconds_floor(seconds).map_err(|e| LwkError::Generic {
                msg: format!("TxSequence from_seconds_floor error: {e}"),
            })?;
        Ok(Arc::new(TxSequence { inner }))
    }

    /// See [`elements::Sequence::from_seconds_ceil`].
    #[uniffi::constructor]
    pub fn from_seconds_ceil(seconds: u32) -> Result<Arc<Self>, LwkError> {
        let inner =
            elements::Sequence::from_seconds_ceil(seconds).map_err(|e| LwkError::Generic {
                msg: format!("TxSequence from_seconds_ceil error: {e}"),
            })?;
        Ok(Arc::new(TxSequence { inner }))
    }

    /// See [`elements::Sequence::to_consensus_u32`].
    pub fn to_consensus_u32(&self) -> u32 {
        self.inner.to_consensus_u32()
    }

    /// See [`elements::Sequence::is_final`].
    pub fn is_final(&self) -> bool {
        self.inner.is_final()
    }

    /// See [`elements::Sequence::is_rbf`].
    pub fn is_rbf(&self) -> bool {
        self.inner.is_rbf()
    }

    /// See [`elements::Sequence::is_relative_lock_time`].
    pub fn is_relative_lock_time(&self) -> bool {
        self.inner.is_relative_lock_time()
    }

    /// See [`elements::Sequence::is_height_locked`].
    pub fn is_height_locked(&self) -> bool {
        self.inner.is_height_locked()
    }

    /// See [`elements::Sequence::is_time_locked`].
    pub fn is_time_locked(&self) -> bool {
        self.inner.is_time_locked()
    }

    /// See [`elements::Sequence::enables_absolute_lock_time`].
    pub fn enables_absolute_lock_time(&self) -> bool {
        self.inner.enables_absolute_lock_time()
    }
}
