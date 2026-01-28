use crate::Error;
use lwk_wollet::elements;
use wasm_bindgen::prelude::*;

/// Transaction input sequence number.
///
/// See [`elements::Sequence`] for more details.
#[wasm_bindgen]
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

#[wasm_bindgen]
impl TxSequence {
    /// Create a TxSequence from a consensus u32 value.
    ///
    /// See [`elements::Sequence::from_consensus`].
    #[wasm_bindgen(constructor)]
    pub fn from_consensus(value: u32) -> TxSequence {
        TxSequence {
            inner: elements::Sequence::from_consensus(value),
        }
    }

    /// Create a TxSequence with value zero.
    ///
    /// See [`elements::Sequence::ZERO`].
    pub fn zero() -> TxSequence {
        TxSequence {
            inner: elements::Sequence::ZERO,
        }
    }

    /// Create a TxSequence with maximum value.
    ///
    /// See [`elements::Sequence::MAX`].
    pub fn max() -> TxSequence {
        TxSequence {
            inner: elements::Sequence::MAX,
        }
    }

    /// Create a TxSequence that enables RBF without lock time.
    ///
    /// See [`elements::Sequence::ENABLE_RBF_NO_LOCKTIME`].
    pub fn enable_rbf_no_locktime() -> TxSequence {
        TxSequence {
            inner: elements::Sequence::ENABLE_RBF_NO_LOCKTIME,
        }
    }

    /// Create a TxSequence that enables lock time without RBF.
    ///
    /// See [`elements::Sequence::ENABLE_LOCKTIME_NO_RBF`].
    pub fn enable_locktime_no_rbf() -> TxSequence {
        TxSequence {
            inner: elements::Sequence::ENABLE_LOCKTIME_NO_RBF,
        }
    }

    /// Create a TxSequence from a block height.
    ///
    /// See [`elements::Sequence::from_height`].
    pub fn from_height(height: u16) -> TxSequence {
        TxSequence {
            inner: elements::Sequence::from_height(height),
        }
    }

    /// Create a TxSequence from 512-second intervals.
    ///
    /// See [`elements::Sequence::from_512_second_intervals`].
    pub fn from_512_second_intervals(intervals: u16) -> TxSequence {
        TxSequence {
            inner: elements::Sequence::from_512_second_intervals(intervals),
        }
    }

    /// Create a TxSequence from seconds, rounding down.
    ///
    /// See [`elements::Sequence::from_seconds_floor`].
    pub fn from_seconds_floor(seconds: u32) -> Result<TxSequence, Error> {
        let inner =
            elements::Sequence::from_seconds_floor(seconds).map_err(|e| {
                Error::Generic(format!("TxSequence from_seconds_floor error: {e}"))
            })?;
        Ok(TxSequence { inner })
    }

    /// Create a TxSequence from seconds, rounding up.
    ///
    /// See [`elements::Sequence::from_seconds_ceil`].
    pub fn from_seconds_ceil(seconds: u32) -> Result<TxSequence, Error> {
        let inner =
            elements::Sequence::from_seconds_ceil(seconds).map_err(|e| {
                Error::Generic(format!("TxSequence from_seconds_ceil error: {e}"))
            })?;
        Ok(TxSequence { inner })
    }

    /// Return the consensus u32 value.
    ///
    /// See [`elements::Sequence::to_consensus_u32`].
    pub fn to_consensus_u32(&self) -> u32 {
        self.inner.to_consensus_u32()
    }

    /// Return true if this sequence is final.
    ///
    /// See [`elements::Sequence::is_final`].
    pub fn is_final(&self) -> bool {
        self.inner.is_final()
    }

    /// Return true if this sequence signals RBF.
    ///
    /// See [`elements::Sequence::is_rbf`].
    pub fn is_rbf(&self) -> bool {
        self.inner.is_rbf()
    }

    /// Return true if this sequence represents a relative lock time.
    ///
    /// See [`elements::Sequence::is_relative_lock_time`].
    pub fn is_relative_lock_time(&self) -> bool {
        self.inner.is_relative_lock_time()
    }

    /// Return true if this sequence is height-locked.
    ///
    /// See [`elements::Sequence::is_height_locked`].
    pub fn is_height_locked(&self) -> bool {
        self.inner.is_height_locked()
    }

    /// Return true if this sequence is time-locked.
    ///
    /// See [`elements::Sequence::is_time_locked`].
    pub fn is_time_locked(&self) -> bool {
        self.inner.is_time_locked()
    }

    /// Return true if this sequence enables absolute lock time.
    ///
    /// See [`elements::Sequence::enables_absolute_lock_time`].
    pub fn enables_absolute_lock_time(&self) -> bool {
        self.inner.enables_absolute_lock_time()
    }

    /// Return the string representation.
    #[wasm_bindgen(js_name = toString)]
    pub fn to_string_js(&self) -> String {
        format!("{}", self.inner)
    }
}

#[cfg(all(test, target_arch = "wasm32"))]
mod tests {
    use super::TxSequence;
    use wasm_bindgen_test::*;

    wasm_bindgen_test_configure!(run_in_browser);

    #[wasm_bindgen_test]
    fn test_tx_sequence_from_consensus() {
        let seq = TxSequence::from_consensus(100);
        assert_eq!(seq.to_consensus_u32(), 100);
    }

    #[wasm_bindgen_test]
    fn test_tx_sequence_zero() {
        let seq = TxSequence::zero();
        assert_eq!(seq.to_consensus_u32(), 0);
    }

    #[wasm_bindgen_test]
    fn test_tx_sequence_max() {
        let seq = TxSequence::max();
        assert_eq!(seq.to_consensus_u32(), 0xFFFFFFFF);
        assert!(seq.is_final());
        assert!(!seq.is_rbf());
    }

    #[wasm_bindgen_test]
    fn test_tx_sequence_rbf() {
        let seq = TxSequence::enable_rbf_no_locktime();
        assert!(seq.is_rbf());
        assert!(!seq.is_final());
    }

    #[wasm_bindgen_test]
    fn test_tx_sequence_locktime() {
        let seq = TxSequence::enable_locktime_no_rbf();
        assert!(seq.enables_absolute_lock_time());
        assert!(!seq.is_rbf());
    }

    #[wasm_bindgen_test]
    fn test_tx_sequence_from_height() {
        let seq = TxSequence::from_height(100);
        assert!(seq.is_relative_lock_time());
        assert!(seq.is_height_locked());
        assert!(!seq.is_time_locked());
    }

    #[wasm_bindgen_test]
    fn test_tx_sequence_from_512_second_intervals() {
        let seq = TxSequence::from_512_second_intervals(10);
        assert!(seq.is_relative_lock_time());
        assert!(seq.is_time_locked());
        assert!(!seq.is_height_locked());
    }

    #[wasm_bindgen_test]
    fn test_tx_sequence_from_seconds() {
        let seq = TxSequence::from_seconds_floor(1024).unwrap();
        assert!(seq.is_relative_lock_time());
        assert!(seq.is_time_locked());

        let seq = TxSequence::from_seconds_ceil(1024).unwrap();
        assert!(seq.is_relative_lock_time());
        assert!(seq.is_time_locked());
    }
}
