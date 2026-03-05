use crate::Error;
use lwk_wollet::elements;
use wasm_bindgen::prelude::*;

/// Transaction input sequence number.
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
    /// Create a sequence from a u32 value.
    #[wasm_bindgen(constructor)]
    pub fn from_consensus(value: u32) -> TxSequence {
        TxSequence {
            inner: elements::Sequence::from_consensus(value),
        }
    }

    /// Zero value sequence.
    ///
    /// This sequence number enables replace-by-fee and lock-time.
    pub fn zero() -> TxSequence {
        TxSequence {
            inner: elements::Sequence::ZERO,
        }
    }

    /// The maximum allowable sequence number.
    ///
    /// This sequence number disables lock-time and replace-by-fee.
    pub fn max() -> TxSequence {
        TxSequence {
            inner: elements::Sequence::MAX,
        }
    }

    /// The sequence number that enables replace-by-fee and absolute lock-time but
    /// disables relative lock-time.
    #[wasm_bindgen(js_name = enableRbfNoLocktime)]
    pub fn enable_rbf_no_locktime() -> TxSequence {
        TxSequence {
            inner: elements::Sequence::ENABLE_RBF_NO_LOCKTIME,
        }
    }

    /// The sequence number that enables absolute lock-time but disables replace-by-fee
    /// and relative lock-time.
    #[wasm_bindgen(js_name = enableLocktimeNoRbf)]
    pub fn enable_locktime_no_rbf() -> TxSequence {
        TxSequence {
            inner: elements::Sequence::ENABLE_LOCKTIME_NO_RBF,
        }
    }

    /// Create a relative lock-time using block height.
    #[wasm_bindgen(js_name = fromHeight)]
    pub fn from_height(height: u16) -> TxSequence {
        TxSequence {
            inner: elements::Sequence::from_height(height),
        }
    }

    /// Create a relative lock-time using time intervals where each interval is equivalent
    /// to 512 seconds.
    #[wasm_bindgen(js_name = from512SecondIntervals)]
    pub fn from_512_second_intervals(intervals: u16) -> TxSequence {
        TxSequence {
            inner: elements::Sequence::from_512_second_intervals(intervals),
        }
    }

    /// Create a relative lock-time from seconds, converting the seconds into 512 second
    /// interval with floor division.
    #[wasm_bindgen(js_name = fromSecondsFloor)]
    pub fn from_seconds_floor(seconds: u32) -> Result<TxSequence, Error> {
        let inner = elements::Sequence::from_seconds_floor(seconds)
            .map_err(|e| Error::Generic(format!("TxSequence from_seconds_floor error: {e}")))?;
        Ok(TxSequence { inner })
    }

    /// Create a relative lock-time from seconds, converting the seconds into 512 second
    /// interval with ceiling division.
    #[wasm_bindgen(js_name = fromSecondsCeil)]
    pub fn from_seconds_ceil(seconds: u32) -> Result<TxSequence, Error> {
        let inner = elements::Sequence::from_seconds_ceil(seconds)
            .map_err(|e| Error::Generic(format!("TxSequence from_seconds_ceil error: {e}")))?;
        Ok(TxSequence { inner })
    }

    /// Returns the inner 32bit integer value of Sequence.
    #[wasm_bindgen(js_name = toConsensusU32)]
    pub fn to_consensus_u32(&self) -> u32 {
        self.inner.to_consensus_u32()
    }

    /// Returns `true` if the sequence number indicates that the transaction is finalised.
    #[wasm_bindgen(js_name = isFinal)]
    pub fn is_final(&self) -> bool {
        self.inner.is_final()
    }

    /// Returns true if the transaction opted-in to BIP125 replace-by-fee.
    #[wasm_bindgen(js_name = isRbf)]
    pub fn is_rbf(&self) -> bool {
        self.inner.is_rbf()
    }

    /// Returns `true` if the sequence has a relative lock-time.
    #[wasm_bindgen(js_name = isRelativeLockTime)]
    pub fn is_relative_lock_time(&self) -> bool {
        self.inner.is_relative_lock_time()
    }

    /// Returns `true` if the sequence number encodes a block based relative lock-time.
    #[wasm_bindgen(js_name = isHeightLocked)]
    pub fn is_height_locked(&self) -> bool {
        self.inner.is_height_locked()
    }

    /// Returns `true` if the sequene number encodes a time interval based relative lock-time.
    #[wasm_bindgen(js_name = isTimeLocked)]
    pub fn is_time_locked(&self) -> bool {
        self.inner.is_time_locked()
    }

    /// Returns `true` if the sequence number enables absolute lock-time.
    #[wasm_bindgen(js_name = enablesAbsoluteLockTime)]
    pub fn enables_absolute_lock_time(&self) -> bool {
        self.inner.enables_absolute_lock_time()
    }
}

#[cfg(all(test, target_arch = "wasm32"))]
mod tests {
    use super::TxSequence;

    use wasm_bindgen_test::*;

    wasm_bindgen_test_configure!(run_in_browser);

    #[wasm_bindgen_test]
    fn test_tx_sequence_roundtrip() {
        assert_eq!(TxSequence::from_consensus(100).to_consensus_u32(), 100);
        assert_eq!(TxSequence::zero().to_consensus_u32(), 0);

        let max_sequence = TxSequence::max();
        assert_eq!(max_sequence.to_consensus_u32(), 0xFFFFFFFF);
        assert!(max_sequence.is_final());
        assert!(!max_sequence.is_rbf());

        let rbf_no_locktime = TxSequence::enable_rbf_no_locktime();
        assert!(rbf_no_locktime.is_rbf());
        assert!(!rbf_no_locktime.is_final());

        let locktime_no_rbf = TxSequence::enable_locktime_no_rbf();
        assert!(locktime_no_rbf.enables_absolute_lock_time());
        assert!(!locktime_no_rbf.is_rbf());

        let height_locked = TxSequence::from_height(100);
        assert!(height_locked.is_relative_lock_time());
        assert!(height_locked.is_height_locked());
        assert!(!height_locked.is_time_locked());

        let time_locked = TxSequence::from_512_second_intervals(10);
        assert!(time_locked.is_relative_lock_time());
        assert!(time_locked.is_time_locked());
        assert!(!time_locked.is_height_locked());

        let seconds_floor = TxSequence::from_seconds_floor(1024).unwrap();
        assert!(seconds_floor.is_relative_lock_time());
        assert!(seconds_floor.is_time_locked());
        assert!(!seconds_floor.is_height_locked());

        let seconds_ceil = TxSequence::from_seconds_ceil(1024).unwrap();
        assert!(seconds_ceil.is_relative_lock_time());
        assert!(seconds_ceil.is_time_locked());
        assert!(!seconds_ceil.is_height_locked());
    }
}
