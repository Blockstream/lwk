/// Default fee rate in sats/kvb (0.1 sat/vb = 100 sats/kvb)
pub const DEFAULT_FEE_RATE: f32 = 100.0;

/// Fee rate in sats/kvb, finite and greater than 0
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct FeeRate(f32);

impl FeeRate {
    /// Create a fee rate from sats/kvb, `None` if not finite or not greater than 0
    pub fn from_sat_kvb(rate: f32) -> Option<Self> {
        (rate.is_finite() && rate > 0.0).then_some(FeeRate(rate))
    }

    /// Fee rate in sats/kvb
    pub fn to_sat_kvb(self) -> f32 {
        self.0
    }
}

impl Default for FeeRate {
    fn default() -> Self {
        FeeRate(DEFAULT_FEE_RATE)
    }
}

/// Calculate the fee from transaction weight and fee rate.
///
/// # Arguments
/// * `weight` - Transaction weight in weight units
/// * `fee_rate` - Fee rate in sats/kvb
pub fn calculate_fee(weight: usize, fee_rate: f32) -> u64 {
    let vsize = weight.div_ceil(4);
    (vsize as f32 * fee_rate / 1000.0).ceil() as u64
}
