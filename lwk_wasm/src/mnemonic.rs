use crate::Error;
use lwk_signer::bip39;
use std::{fmt::Display, str::FromStr};
use wasm_bindgen::prelude::*;

/// A mnemonic secret code used as a master secret for a bip39 wallet.
///
/// Supported number of words are 12, 15, 18, 21, and 24.
#[wasm_bindgen]
#[derive(PartialEq, Eq, Debug)]
pub struct Mnemonic {
    inner: bip39::Mnemonic,
}

impl From<bip39::Mnemonic> for Mnemonic {
    fn from(inner: bip39::Mnemonic) -> Self {
        Self { inner }
    }
}

impl From<Mnemonic> for bip39::Mnemonic {
    fn from(mnemonic: Mnemonic) -> Self {
        mnemonic.inner
    }
}

impl From<&Mnemonic> for bip39::Mnemonic {
    fn from(mnemonic: &Mnemonic) -> Self {
        mnemonic.inner.clone()
    }
}

impl Display for Mnemonic {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.inner)
    }
}

#[wasm_bindgen]
impl Mnemonic {
    /// Creates a Mnemonic
    #[wasm_bindgen(constructor)]
    pub fn new(s: &str) -> Result<Mnemonic, Error> {
        let inner = bip39::Mnemonic::from_str(s)?;
        Ok(inner.into())
    }

    /// Return the string representation of the Mnemonic.
    /// This representation can be used to recreate the Mnemonic via `new()`
    ///
    /// Note this is secret information, do not log it.
    #[wasm_bindgen(js_name = toString)]
    pub fn to_string_js(&self) -> String {
        format!("{self}")
    }

    /// Creates a Mnemonic from entropy, at least 16 bytes are needed.
    #[wasm_bindgen(js_name = fromEntropy)]
    pub fn from_entropy(b: &[u8]) -> Result<Mnemonic, Error> {
        let inner = bip39::Mnemonic::from_entropy(b)?;
        Ok(inner.into())
    }

    /// Creates a random Mnemonic of given words (12,15,18,21,24)
    #[wasm_bindgen(js_name = fromRandom)]
    pub fn from_random(word_count: usize) -> Result<Mnemonic, Error> {
        let inner = bip39::Mnemonic::generate(word_count)?;
        Ok(inner.into())
    }
}

#[cfg(all(test, target_arch = "wasm32"))]
mod tests {
    use crate::Mnemonic;
    use lwk_signer::bip39;
    use std::str::FromStr;
    use wasm_bindgen_test::*;

    wasm_bindgen_test_configure!(run_in_browser);

    #[wasm_bindgen_test]
    fn mnemonic() {
        let mnemonic_str = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
        let mnemonic_bip39 = bip39::Mnemonic::from_str(mnemonic_str).unwrap();
        let from_bip39: Mnemonic = mnemonic_bip39.into();
        let mnemonic = Mnemonic::new(mnemonic_str).unwrap();
        assert_eq!(mnemonic_str, mnemonic.to_string());
        assert_eq!(from_bip39, mnemonic);

        let mnemonic_entropy = Mnemonic::from_entropy(&[1u8; 32]).unwrap();
        assert_eq!(mnemonic_entropy.to_string(), "absurd amount doctor acoustic avoid letter advice cage absurd amount doctor acoustic avoid letter advice cage absurd amount doctor acoustic avoid letter advice comic");
        let mnemonic_entropy = Mnemonic::from_entropy(&[1u8; 16]).unwrap();
        assert_eq!(
            mnemonic_entropy.to_string(),
            "absurd amount doctor acoustic avoid letter advice cage absurd amount doctor adjust"
        );

        let err = Mnemonic::from_entropy(&[1u8; 15]).unwrap_err();
        assert_eq!(
            err.to_string(),
            "entropy was not between 128-256 bits or not a multiple of 32 bits: 120 bits"
        );

        let mnemonic_random = Mnemonic::from_random(12).unwrap();
        assert_eq!(mnemonic_random.to_string().split(' ').count(), 12);
        let err = Mnemonic::from_random(11).unwrap_err();
        assert_eq!(
            err.to_string(),
            "mnemonic has an invalid word count: 11. Word count must be 12, 15, 18, 21, or 24"
        );
    }

    #[wasm_bindgen_test]
    #[ignore]
    fn test_mnemonic_statistical_entropy() {
        let sample_size = 10_000;
        let mut byte_counts = [0u64; 256];
        let mut bit_ones = 0u64;
        let mut generated = std::collections::HashSet::with_capacity(sample_size);

        for _ in 0..sample_size {
            let mnemonic = Mnemonic::from_random(12).unwrap();
            let entropy = mnemonic.inner.to_entropy(); // 16 bytes (128 bits)
            let entropy_bytes: [u8; 16] = entropy.clone().try_into().unwrap();

            assert!(
                generated.insert(entropy_bytes),
                "Collision detected! Entropy source is deterministic or has low cardinality."
            );

            for &byte in &entropy {
                byte_counts[byte as usize] += 1;
                bit_ones += byte.count_ones() as u64;
            }
        }

        let total_bytes = sample_size * 16;
        let total_bits = total_bytes * 8;

        // Monobit test (frequency check of ones/zeros)
        let expected_ones = total_bits as f64 / 2.0;
        let std_dev = (total_bits as f64).sqrt() / 2.0;
        let observed_ones = bit_ones as f64;
        let z_score = (observed_ones - expected_ones).abs() / std_dev;
        assert!(
            z_score < 4.0,
            "Monobit test failed: z-score was {}",
            z_score
        );

        // Chi-squared test on byte frequencies
        let expected_count = total_bytes as f64 / 256.0;
        let mut chi_squared = 0.0;
        for &count in &byte_counts {
            let diff = count as f64 - expected_count;
            chi_squared += (diff * diff) / expected_count;
        }

        // For 255 degrees of freedom:
        // - Chi-squared critical value at 99.9% confidence is ~323
        // - Chi-squared critical value at 0.1% confidence is ~197
        assert!(
            (190.0..330.0).contains(&chi_squared),
            "Chi-squared test failed: value was {} (expected [190, 330])",
            chi_squared
        );

        // Shannon entropy
        let mut entropy_val = 0.0;
        for &count in &byte_counts {
            if count > 0 {
                let p = count as f64 / total_bytes as f64;
                entropy_val -= p * p.log2();
            }
        }
        assert!(
            entropy_val > 7.99,
            "Shannon entropy too low: {} (expected > 7.99)",
            entropy_val
        );
    }
}
