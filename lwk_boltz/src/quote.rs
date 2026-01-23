//! Swap quote functionality for calculating fees before creating a swap.
//!
//! This module provides types and builders for getting fee estimates without
//! actually creating a swap.

use boltz_client::boltz::{
    GetChainPairsResponse, GetReversePairsResponse, GetSubmarinePairsResponse,
};

use crate::Error;

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
/// let quote = session.quote(25000)
///     .from(SwapAsset::LightningBtc)
///     .to(SwapAsset::Liquid)
///     .build()?;
/// ```
pub struct QuoteBuilder {
    amount: u64,
    from: Option<SwapAsset>,
    to: Option<SwapAsset>,
    // Cloned pairs data for binding compatibility (no lifetimes)
    submarine_pairs: GetSubmarinePairsResponse,
    reverse_pairs: GetReversePairsResponse,
    chain_pairs: GetChainPairsResponse,
}

impl QuoteBuilder {
    pub(crate) fn new(
        amount: u64,
        submarine_pairs: GetSubmarinePairsResponse,
        reverse_pairs: GetReversePairsResponse,
        chain_pairs: GetChainPairsResponse,
    ) -> Self {
        Self {
            amount,
            from: None,
            to: None,
            submarine_pairs,
            reverse_pairs,
            chain_pairs,
        }
    }

    /// Set the source asset for the swap
    pub fn from(mut self, asset: SwapAsset) -> Self {
        self.from = Some(asset);
        self
    }

    /// Set the destination asset for the swap
    pub fn to(mut self, asset: SwapAsset) -> Self {
        self.to = Some(asset);
        self
    }

    /// Build the quote, calculating fees and receive amount
    ///
    /// # Fee Calculation
    ///
    /// The fee calculation follows the Boltz web app behavior:
    ///
    /// - **Reverse swaps** (Lightning → onchain): User pays the `claim` fee only
    ///   (the lockup is on Lightning side, handled by Boltz)
    /// - **Submarine swaps** (onchain → Lightning): User pays the `minerFees` value
    /// - **Chain swaps** (BTC ↔ L-BTC): User pays `server + claim` fees
    ///   (server fee covers Boltz's lockup on the destination chain)
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - `from` or `to` was not set
    /// - The swap pair is not supported
    /// - The pair is not available from the Boltz API
    pub fn build(self) -> Result<Quote, Error> {
        let from = self.from.ok_or(Error::MissingQuoteParam("from"))?;
        let to = self.to.ok_or(Error::MissingQuoteParam("to"))?;

        match (from, to) {
            (SwapAsset::LightningBtc, SwapAsset::Liquid) => {
                // Reverse swap: Lightning -> Liquid
                // User receives onchain, so they pay the claim fee only
                let pair = self
                    .reverse_pairs
                    .get_btc_to_lbtc_pair()
                    .ok_or(Error::PairNotAvailable)?;
                let boltz_fee = pair.fees.boltz(self.amount);
                // Only claim fee - lockup is on Lightning side (Boltz's cost)
                let network_fee = pair.fees.claim_estimate();
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
                // User pays server fee (for Boltz's L-BTC lockup) + claim fee (to claim L-BTC)
                let pair = self
                    .chain_pairs
                    .get_btc_to_lbtc_pair()
                    .ok_or(Error::PairNotAvailable)?;
                let boltz_fee = pair.fees.boltz(self.amount);
                // Server + claim only (user's lockup fee is part of what they send)
                let network_fee = pair.fees.server() + pair.fees.claim_estimate();
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
                // User pays server fee (for Boltz's BTC lockup) + claim fee (to claim BTC)
                let pair = self
                    .chain_pairs
                    .get_lbtc_to_btc_pair()
                    .ok_or(Error::PairNotAvailable)?;
                let boltz_fee = pair.fees.boltz(self.amount);
                // Server + claim only (user's lockup fee is part of what they send)
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

    fn load_test_pairs() -> (
        GetSubmarinePairsResponse,
        GetReversePairsResponse,
        GetChainPairsResponse,
    ) {
        let reverse_pairs: GetReversePairsResponse =
            serde_json::from_str(include_str!("../tests/data/swap-reverse.json")).unwrap();
        let submarine_pairs: GetSubmarinePairsResponse =
            serde_json::from_str(include_str!("../tests/data/swap-submarine.json")).unwrap();
        let chain_pairs: GetChainPairsResponse =
            serde_json::from_str(include_str!("../tests/data/swap-chain.json")).unwrap();
        (submarine_pairs, reverse_pairs, chain_pairs)
    }

    #[test]
    fn test_quote_builder_reverse() {
        let (submarine_pairs, reverse_pairs, chain_pairs) = load_test_pairs();

        // Test reverse swap: Lightning -> Liquid (25000 sats)
        let quote = QuoteBuilder::new(25000, submarine_pairs, reverse_pairs, chain_pairs)
            .from(SwapAsset::LightningBtc)
            .to(SwapAsset::Liquid)
            .build()
            .unwrap();

        // From swap-reverse.json BTC -> L-BTC pair:
        // percentage: 0.25, claim: 20, lockup: 27
        // boltz_fee = ceil(0.25 / 100 * 25000) = ceil(62.5) = 63
        // network_fee = claim only = 20 (user claims onchain)
        // receive_amount = 25000 - 63 - 20 = 24917
        assert_eq!(quote.boltz_fee, 63);
        assert_eq!(quote.network_fee, 20);
        assert_eq!(quote.receive_amount, 24917);
        assert_eq!(quote.min, 100);
        assert_eq!(quote.max, 25_000_000);
    }

    #[test]
    fn test_quote_builder_submarine() {
        let (submarine_pairs, reverse_pairs, chain_pairs) = load_test_pairs();

        // Test submarine swap: Liquid -> Lightning (25000 sats) to match screenshot
        let quote = QuoteBuilder::new(25000, submarine_pairs, reverse_pairs, chain_pairs)
            .from(SwapAsset::Liquid)
            .to(SwapAsset::LightningBtc)
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
        let (submarine_pairs, reverse_pairs, chain_pairs) = load_test_pairs();

        // Test chain swap: L-BTC -> BTC (25000 sats) to match screenshot
        let quote = QuoteBuilder::new(25000, submarine_pairs, reverse_pairs, chain_pairs)
            .from(SwapAsset::Liquid)
            .to(SwapAsset::OnchainBtc)
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
        let (submarine_pairs, reverse_pairs, chain_pairs) = load_test_pairs();

        // Test chain swap: BTC -> L-BTC (50000 sats)
        let quote = QuoteBuilder::new(50000, submarine_pairs, reverse_pairs, chain_pairs)
            .from(SwapAsset::OnchainBtc)
            .to(SwapAsset::Liquid)
            .build()
            .unwrap();

        // From swap-chain.json BTC -> L-BTC pair:
        // percentage: 0.1, server: 480, user.claim: 20, user.lockup: 462
        // boltz_fee = ceil(0.1 / 100 * 50000) = ceil(50) = 50
        // network_fee = server + claim = 480 + 20 = 500
        // receive_amount = 50000 - 50 - 500 = 49450
        assert_eq!(quote.boltz_fee, 50);
        assert_eq!(quote.network_fee, 500);
        assert_eq!(quote.receive_amount, 49450);
        assert_eq!(quote.min, 25000);
        assert_eq!(quote.max, 25_000_000);
    }

    #[test]
    fn test_quote_builder_invalid_pair() {
        let (submarine_pairs, reverse_pairs, chain_pairs) = load_test_pairs();

        // Test invalid pair: Lightning -> Lightning
        let result = QuoteBuilder::new(
            25000,
            submarine_pairs.clone(),
            reverse_pairs.clone(),
            chain_pairs.clone(),
        )
        .from(SwapAsset::LightningBtc)
        .to(SwapAsset::LightningBtc)
        .build();

        assert!(matches!(result, Err(Error::InvalidSwapPair { .. })));

        // Test invalid pair: OnchainBtc -> OnchainBtc
        let result = QuoteBuilder::new(25000, submarine_pairs, reverse_pairs, chain_pairs)
            .from(SwapAsset::OnchainBtc)
            .to(SwapAsset::OnchainBtc)
            .build();

        assert!(matches!(result, Err(Error::InvalidSwapPair { .. })));
    }

    #[test]
    fn test_quote_builder_missing_params() {
        let (submarine_pairs, reverse_pairs, chain_pairs) = load_test_pairs();

        // Test missing 'from' param
        let result = QuoteBuilder::new(
            25000,
            submarine_pairs.clone(),
            reverse_pairs.clone(),
            chain_pairs.clone(),
        )
        .to(SwapAsset::Liquid)
        .build();

        assert!(matches!(result, Err(Error::MissingQuoteParam("from"))));

        // Test missing 'to' param
        let result = QuoteBuilder::new(25000, submarine_pairs, reverse_pairs, chain_pairs)
            .from(SwapAsset::LightningBtc)
            .build();

        assert!(matches!(result, Err(Error::MissingQuoteParam("to"))));
    }
}
