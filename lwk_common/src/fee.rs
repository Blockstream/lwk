/// Default fee rate in sats/kvb (0.1 sat/vb = 100 sats/kvb)
pub const DEFAULT_FEE_RATE: f32 = 100.0;

/// Calculate the fee from transaction weight and fee rate.
///
/// # Arguments
/// * `weight` - Transaction weight in weight units
/// * `fee_rate` - Fee rate in sats/kvb
#[must_use]
pub fn calculate_fee(weight: usize, fee_rate: f32) -> u64 {
    let vsize = weight.div_ceil(4);
    (vsize as f32 * fee_rate / 1000.0).ceil() as u64
}
