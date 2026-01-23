//! Swap quote functionality for calculating fees before creating a swap.
//!
//! This module provides types and builders for getting fee estimates without
//! actually creating a swap.

use crate::SwapInfo;

use crate::Error;

/// Extra fee added when claiming on Liquid to cover the "uncooperative claim" scenario.
///
/// When a cooperative Schnorr signature claim fails, Boltz falls back to using the
/// script path which requires a slightly larger transaction.
///
/// From Boltz web app:
/// <https://github.com/BoltzExchange/boltz-web-app/blob/f3f14669822dc0e4a7fb950964087a2d5b5cd06d/src/context/Global.tsx#L38>
const LIQUID_UNCOOPERATIVE_EXTRA: u64 = 3;

/// Asset type for swap quotes
///
/// Used to specify the source and destination of a swap when creating a quote.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SwapAsset {
    /// Lightning Bitcoin (for reverse/submarine swaps)
    LightningBtc,
    /// Onchain Bitcoin (for chain swaps)
    OnchainBtc,
    /// Liquid Bitcoin (onchain)
    Liquid,
}

/// Quote result containing fee breakdown for a swap
#[derive(Debug, Clone)]
pub struct Quote {
    /// Amount the user will receive after fees
    pub receive_amount: u64,
    /// Network/miner fee in satoshis
    pub network_fee: u64,
    /// Boltz service fee in satoshis
    pub boltz_fee: u64,
    /// Minimum amount for this swap pair
    pub min: u64,
    /// Maximum amount for this swap pair
    pub max: u64,
}

/// Builder for creating swap quotes
///
/// Created via [`crate::BoltzSession::quote`].
///
/// # Example
/// ```ignore
/// let quote = session
///     .quote(25000)
///     .await
///     .send(SwapAsset::LightningBtc)
///     .receive(SwapAsset::Liquid)
///     .build()?;
/// ```
pub struct QuoteBuilder {
    amount: u64,
    from: Option<SwapAsset>,
    to: Option<SwapAsset>,
    // Cloned pairs data for binding compatibility (no lifetimes)
    swap_info: SwapInfo,
}

impl QuoteBuilder {
    pub(crate) fn new(amount: u64, swap_info: SwapInfo) -> Self {
        Self {
            amount,
            from: None,
            to: None,
            swap_info,
        }
    }

    /// Set the source asset for the swap
    pub fn send(mut self, asset: SwapAsset) -> Self {
        self.from = Some(asset);
        self
    }

    /// Set the destination asset for the swap
    pub fn receive(mut self, asset: SwapAsset) -> Self {
        self.to = Some(asset);
        self
    }

    /// Build the quote, calculating fees and receive amount
    ///
    /// # Fee Calculation
    ///
    /// The fee calculation follows the Boltz web app behavior
    /// (see `boltz-web-app/src/components/Fees.tsx`):
    ///
    /// - **Reverse swaps** (Lightning → onchain): User pays `claim + lockup` fees
    /// - **Submarine swaps** (onchain → Lightning): User pays the `minerFees` value
    /// - **Chain swaps** (BTC ↔ L-BTC): User pays `server + user.claim` fees
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - `from` or `to` was not set
    /// - The swap pair is not supported
    /// - The pair is not available from the Boltz API
    pub fn build(self) -> Result<Quote, Error> {
        let from = self.from.ok_or(Error::MissingQuoteParam("send"))?;
        let to = self.to.ok_or(Error::MissingQuoteParam("receive"))?;

        match (from, to) {
            (SwapAsset::LightningBtc, SwapAsset::Liquid) => {
                // Reverse swap: Lightning -> Liquid
                // From Boltz web app: fee = claim + lockup + LIQUID_UNCOOPERATIVE_EXTRA
                let pair = self
                    .swap_info
                    .reverse_pairs
                    .get_btc_to_lbtc_pair()
                    .ok_or(Error::PairNotAvailable)?;
                let boltz_fee = pair.fees.boltz(self.amount);
                let network_fee =
                    pair.fees.claim_estimate() + pair.fees.lockup() + LIQUID_UNCOOPERATIVE_EXTRA;
                Ok(Quote {
                    receive_amount: self.amount.saturating_sub(boltz_fee + network_fee),
                    network_fee,
                    boltz_fee,
                    min: pair.limits.minimal,
                    max: pair.limits.maximal,
                })
            }
            (SwapAsset::Liquid, SwapAsset::LightningBtc) => {
                // Submarine swap: Liquid -> Lightning
                let pair = self
                    .swap_info
                    .submarine_pairs
                    .get_lbtc_to_btc_pair()
                    .ok_or(Error::PairNotAvailable)?;
                let boltz_fee = pair.fees.boltz(self.amount);
                let network_fee = pair.fees.network();
                Ok(Quote {
                    receive_amount: self.amount.saturating_sub(boltz_fee + network_fee),
                    network_fee,
                    boltz_fee,
                    min: pair.limits.minimal,
                    max: pair.limits.maximal,
                })
            }
            (SwapAsset::OnchainBtc, SwapAsset::Liquid) => {
                // Chain swap: BTC -> L-BTC
                // From Boltz web app: fee = server + user.claim + LIQUID_UNCOOPERATIVE_EXTRA
                let pair = self
                    .swap_info
                    .chain_pairs
                    .get_btc_to_lbtc_pair()
                    .ok_or(Error::PairNotAvailable)?;
                let boltz_fee = pair.fees.boltz(self.amount);
                let network_fee =
                    pair.fees.server() + pair.fees.claim_estimate() + LIQUID_UNCOOPERATIVE_EXTRA;
                Ok(Quote {
                    receive_amount: self.amount.saturating_sub(boltz_fee + network_fee),
                    network_fee,
                    boltz_fee,
                    min: pair.limits.minimal,
                    max: pair.limits.maximal,
                })
            }
            (SwapAsset::Liquid, SwapAsset::OnchainBtc) => {
                // Chain swap: L-BTC -> BTC
                // From Boltz web app: fee = server + user.claim
                let pair = self
                    .swap_info
                    .chain_pairs
                    .get_lbtc_to_btc_pair()
                    .ok_or(Error::PairNotAvailable)?;
                let boltz_fee = pair.fees.boltz(self.amount);
                let network_fee = pair.fees.server() + pair.fees.claim_estimate();
                Ok(Quote {
                    receive_amount: self.amount.saturating_sub(boltz_fee + network_fee),
                    network_fee,
                    boltz_fee,
                    min: pair.limits.minimal,
                    max: pair.limits.maximal,
                })
            }
            _ => Err(Error::InvalidSwapPair { from, to }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn load_test_pairs() -> SwapInfo {
        let reverse_pairs =
            serde_json::from_str(include_str!("../tests/data/swap-reverse.json")).unwrap();
        let submarine_pairs =
            serde_json::from_str(include_str!("../tests/data/swap-submarine.json")).unwrap();
        let chain_pairs =
            serde_json::from_str(include_str!("../tests/data/swap-chain.json")).unwrap();
        SwapInfo {
            reverse_pairs,
            submarine_pairs,
            chain_pairs,
        }
    }

    #[test]
    fn test_quote_builder_reverse() {
        // Test reverse swap: Lightning -> Liquid (25000 sats)
        let quote = QuoteBuilder::new(25000, load_test_pairs())
            .send(SwapAsset::LightningBtc)
            .receive(SwapAsset::Liquid)
            .build()
            .unwrap();

        // From swap-reverse.json BTC -> L-BTC pair:
        // percentage: 0.25, claim: 20, lockup: 27
        // boltz_fee = ceil(0.25 / 100 * 25000) = ceil(62.5) = 63
        // network_fee = claim + lockup + LIQUID_UNCOOPERATIVE_EXTRA = 20 + 27 + 3 = 50
        // receive_amount = 25000 - 63 - 50 = 24887
        // This matches the Boltz web app screenshot exactly!
        assert_eq!(quote.boltz_fee, 63);
        assert_eq!(quote.network_fee, 50);
        assert_eq!(quote.receive_amount, 24887);
        assert_eq!(quote.min, 100);
        assert_eq!(quote.max, 25_000_000);
    }

    #[test]
    fn test_quote_builder_submarine() {
        // Test submarine swap: Liquid -> Lightning (25000 sats) to match screenshot
        let quote = QuoteBuilder::new(25000, load_test_pairs())
            .send(SwapAsset::Liquid)
            .receive(SwapAsset::LightningBtc)
            .build()
            .unwrap();

        // From swap-submarine.json L-BTC -> BTC pair:
        // percentage: 0.1, minerFees: 19
        // boltz_fee = ceil(0.1 / 100 * 25000) = ceil(25) = 25
        // network_fee = 19
        // receive_amount = 25000 - 25 - 19 = 24956
        // This matches the screenshot exactly!
        assert_eq!(quote.boltz_fee, 25);
        assert_eq!(quote.network_fee, 19);
        assert_eq!(quote.receive_amount, 24956);
        assert_eq!(quote.min, 1000);
        assert_eq!(quote.max, 25_000_000);
    }

    #[test]
    fn test_quote_builder_chain_lbtc_to_btc() {
        // Test chain swap: L-BTC -> BTC (25000 sats) to match screenshot
        let quote = QuoteBuilder::new(25000, load_test_pairs())
            .send(SwapAsset::Liquid)
            .receive(SwapAsset::OnchainBtc)
            .build()
            .unwrap();

        // From swap-chain.json L-BTC -> BTC pair:
        // percentage: 0.1, server: 481, user.claim: 333, user.lockup: 27
        // boltz_fee = ceil(0.1 / 100 * 25000) = ceil(25) = 25
        // network_fee = server + claim = 481 + 333 = 814
        // receive_amount = 25000 - 25 - 814 = 24161
        // This matches the screenshot exactly!
        assert_eq!(quote.boltz_fee, 25);
        assert_eq!(quote.network_fee, 814);
        assert_eq!(quote.receive_amount, 24161);
        assert_eq!(quote.min, 25000);
        assert_eq!(quote.max, 25_000_000);
    }

    #[test]
    fn test_quote_builder_chain_btc_to_lbtc() {
        // Test chain swap: BTC -> L-BTC (50000 sats)
        let quote = QuoteBuilder::new(50000, load_test_pairs())
            .send(SwapAsset::OnchainBtc)
            .receive(SwapAsset::Liquid)
            .build()
            .unwrap();

        // From swap-chain.json BTC -> L-BTC pair:
        // percentage: 0.1, server: 480, user.claim: 20, user.lockup: 462
        // boltz_fee = ceil(0.1 / 100 * 50000) = ceil(50) = 50
        // network_fee = server + claim + LIQUID_UNCOOPERATIVE_EXTRA = 480 + 20 + 3 = 503
        // receive_amount = 50000 - 50 - 503 = 49447
        assert_eq!(quote.boltz_fee, 50);
        assert_eq!(quote.network_fee, 503);
        assert_eq!(quote.receive_amount, 49447);
        assert_eq!(quote.min, 25000);
        assert_eq!(quote.max, 25_000_000);
    }

    #[test]
    fn test_quote_builder_invalid_pair() {
        // Test invalid pair: Lightning -> Lightning
        let swap_info = load_test_pairs();
        let result = QuoteBuilder::new(25000, swap_info.clone())
            .send(SwapAsset::LightningBtc)
            .receive(SwapAsset::LightningBtc)
            .build();

        assert!(matches!(result, Err(Error::InvalidSwapPair { .. })));

        // Test invalid pair: OnchainBtc -> OnchainBtc
        let result = QuoteBuilder::new(25000, swap_info)
            .send(SwapAsset::OnchainBtc)
            .receive(SwapAsset::OnchainBtc)
            .build();

        assert!(matches!(result, Err(Error::InvalidSwapPair { .. })));
    }

    #[test]
    fn test_quote_builder_missing_params() {
        // Test missing 'send' param
        let swap_info = load_test_pairs();
        let result = QuoteBuilder::new(25000, swap_info.clone())
            .receive(SwapAsset::Liquid)
            .build();

        assert!(matches!(result, Err(Error::MissingQuoteParam("send"))));

        // Test missing 'receive' param
        let result = QuoteBuilder::new(25000, swap_info)
            .send(SwapAsset::LightningBtc)
            .build();

        assert!(matches!(result, Err(Error::MissingQuoteParam("receive"))));
    }
}
