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
    Lightning,
    /// Onchain Bitcoin (for chain swaps)
    Onchain,
    /// Liquid Bitcoin (onchain)
    Liquid,
}

/// Quote result containing fee breakdown for a swap
#[derive(Debug, Clone)]
pub struct Quote {
    /// Amount the user sends (before fees)
    pub send_amount: u64,
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

/// Mode for quote calculation
#[derive(Debug, Clone, Copy)]
enum QuoteMode {
    /// Calculate receive amount from send amount
    BySendAmount(u64),
    /// Calculate send amount from receive amount
    ByReceiveAmount(u64),
}

/// Builder for creating swap quotes
///
/// Created via [`crate::BoltzSession::quote`] or [`crate::BoltzSession::quote_receive`].
///
/// # Example - Quote by send amount
/// ```ignore
/// let quote = session
///     .quote(25000)
///     .await
///     .send(SwapAsset::Lightning)
///     .receive(SwapAsset::Liquid)
///     .build()?;
/// // quote.receive_amount tells you how much you'll get
/// ```
///
/// # Example - Quote by receive amount
/// ```ignore
/// let quote = session
///     .quote_receive(24887)
///     .await
///     .send(SwapAsset::Lightning)
///     .receive(SwapAsset::Liquid)
///     .build()?;
/// // quote.send_amount tells you how much you need to send
/// ```
pub struct QuoteBuilder {
    mode: QuoteMode,
    from: Option<SwapAsset>,
    to: Option<SwapAsset>,
    // Cloned pairs data for binding compatibility (no lifetimes)
    swap_info: SwapInfo,
}

impl QuoteBuilder {
    /// Create a quote builder to calculate receive amount from a send amount
    pub(crate) fn new_send(send_amount: u64, swap_info: SwapInfo) -> Self {
        Self {
            mode: QuoteMode::BySendAmount(send_amount),
            from: None,
            to: None,
            swap_info,
        }
    }

    /// Create a quote builder to calculate send amount from a desired receive amount
    pub(crate) fn new_receive(receive_amount: u64, swap_info: SwapInfo) -> Self {
        Self {
            mode: QuoteMode::ByReceiveAmount(receive_amount),
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

    /// Build the quote, calculating fees and amounts
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
            (SwapAsset::Lightning, SwapAsset::Liquid) => {
                // Reverse swap: Lightning -> Liquid
                // From Boltz web app: fee = claim + lockup + LIQUID_UNCOOPERATIVE_EXTRA
                let pair = self
                    .swap_info
                    .reverse_pairs
                    .get_btc_to_lbtc_pair()
                    .ok_or(Error::PairNotAvailable)?;
                let network_fee =
                    pair.fees.claim_estimate() + pair.fees.lockup() + LIQUID_UNCOOPERATIVE_EXTRA;
                let percentage = pair.fees.percentage;

                let (send_amount, receive_amount, boltz_fee) = match self.mode {
                    QuoteMode::BySendAmount(send) => {
                        let bf = pair.fees.boltz(send);
                        let recv = send.saturating_sub(bf + network_fee);
                        (send, recv, bf)
                    }
                    QuoteMode::ByReceiveAmount(recv) => {
                        let send = calculate_send_amount(recv, network_fee, percentage);
                        let bf = pair.fees.boltz(send);
                        (send, recv, bf)
                    }
                };

                Ok(Quote {
                    send_amount,
                    receive_amount,
                    network_fee,
                    boltz_fee,
                    min: pair.limits.minimal,
                    max: pair.limits.maximal,
                })
            }
            (SwapAsset::Liquid, SwapAsset::Lightning) => {
                // Submarine swap: Liquid -> Lightning
                let pair = self
                    .swap_info
                    .submarine_pairs
                    .get_lbtc_to_btc_pair()
                    .ok_or(Error::PairNotAvailable)?;
                let network_fee = pair.fees.network();
                let percentage = pair.fees.percentage;

                let (send_amount, receive_amount, boltz_fee) = match self.mode {
                    QuoteMode::BySendAmount(send) => {
                        let bf = pair.fees.boltz(send);
                        let recv = send.saturating_sub(bf + network_fee);
                        (send, recv, bf)
                    }
                    QuoteMode::ByReceiveAmount(recv) => {
                        let send = calculate_send_amount(recv, network_fee, percentage);
                        let bf = pair.fees.boltz(send);
                        (send, recv, bf)
                    }
                };

                Ok(Quote {
                    send_amount,
                    receive_amount,
                    network_fee,
                    boltz_fee,
                    min: pair.limits.minimal,
                    max: pair.limits.maximal,
                })
            }
            (SwapAsset::Onchain, SwapAsset::Liquid) => {
                // Chain swap: BTC -> L-BTC
                // From Boltz web app: fee = server + user.claim + LIQUID_UNCOOPERATIVE_EXTRA
                let pair = self
                    .swap_info
                    .chain_pairs
                    .get_btc_to_lbtc_pair()
                    .ok_or(Error::PairNotAvailable)?;
                let network_fee =
                    pair.fees.server() + pair.fees.claim_estimate() + LIQUID_UNCOOPERATIVE_EXTRA;
                let percentage = pair.fees.percentage;

                let (send_amount, receive_amount, boltz_fee) = match self.mode {
                    QuoteMode::BySendAmount(send) => {
                        let bf = pair.fees.boltz(send);
                        let recv = send.saturating_sub(bf + network_fee);
                        (send, recv, bf)
                    }
                    QuoteMode::ByReceiveAmount(recv) => {
                        let send = calculate_send_amount(recv, network_fee, percentage);
                        let bf = pair.fees.boltz(send);
                        (send, recv, bf)
                    }
                };

                Ok(Quote {
                    send_amount,
                    receive_amount,
                    network_fee,
                    boltz_fee,
                    min: pair.limits.minimal,
                    max: pair.limits.maximal,
                })
            }
            (SwapAsset::Liquid, SwapAsset::Onchain) => {
                // Chain swap: L-BTC -> BTC
                // From Boltz web app: fee = server + user.claim
                let pair = self
                    .swap_info
                    .chain_pairs
                    .get_lbtc_to_btc_pair()
                    .ok_or(Error::PairNotAvailable)?;
                let network_fee = pair.fees.server() + pair.fees.claim_estimate();
                let percentage = pair.fees.percentage;

                let (send_amount, receive_amount, boltz_fee) = match self.mode {
                    QuoteMode::BySendAmount(send) => {
                        let bf = pair.fees.boltz(send);
                        let recv = send.saturating_sub(bf + network_fee);
                        (send, recv, bf)
                    }
                    QuoteMode::ByReceiveAmount(recv) => {
                        let send = calculate_send_amount(recv, network_fee, percentage);
                        let bf = pair.fees.boltz(send);
                        (send, recv, bf)
                    }
                };

                Ok(Quote {
                    send_amount,
                    receive_amount,
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

/// Calculate the send amount from a desired receive amount
///
/// Given: `receive = send - ceil(percentage * send / 100) - network_fee`
/// Solve for `send`: `send = ceil((receive + network_fee) / (1 - percentage/100))`
///
fn calculate_send_amount(receive_amount: u64, network_fee: u64, percentage: f64) -> u64 {
    let base = receive_amount + network_fee;
    let rate = 1.0 - percentage / 100.0;
    (base as f64 / rate).ceil() as u64
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
        let quote = QuoteBuilder::new_send(25000, load_test_pairs())
            .send(SwapAsset::Lightning)
            .receive(SwapAsset::Liquid)
            .build()
            .unwrap();

        // From swap-reverse.json BTC -> L-BTC pair:
        // percentage: 0.25, claim: 20, lockup: 27
        // boltz_fee = ceil(0.25 / 100 * 25000) = ceil(62.5) = 63
        // network_fee = claim + lockup + LIQUID_UNCOOPERATIVE_EXTRA = 20 + 27 + 3 = 50
        // receive_amount = 25000 - 63 - 50 = 24887
        // This matches the Boltz web app screenshot exactly!
        assert_eq!(quote.send_amount, 25000);
        assert_eq!(quote.boltz_fee, 63);
        assert_eq!(quote.network_fee, 50);
        assert_eq!(quote.receive_amount, 24887);
        assert_eq!(quote.min, 100);
        assert_eq!(quote.max, 25_000_000);
    }

    #[test]
    fn test_quote_builder_submarine() {
        // Test submarine swap: Liquid -> Lightning (25000 sats) to match screenshot
        let quote = QuoteBuilder::new_send(25000, load_test_pairs())
            .send(SwapAsset::Liquid)
            .receive(SwapAsset::Lightning)
            .build()
            .unwrap();

        // From swap-submarine.json L-BTC -> BTC pair:
        // percentage: 0.1, minerFees: 19
        // boltz_fee = ceil(0.1 / 100 * 25000) = ceil(25) = 25
        // network_fee = 19
        // receive_amount = 25000 - 25 - 19 = 24956
        // This matches the screenshot exactly!
        assert_eq!(quote.send_amount, 25000);
        assert_eq!(quote.boltz_fee, 25);
        assert_eq!(quote.network_fee, 19);
        assert_eq!(quote.receive_amount, 24956);
        assert_eq!(quote.min, 1000);
        assert_eq!(quote.max, 25_000_000);
    }

    #[test]
    fn test_quote_builder_chain_lbtc_to_btc() {
        // Test chain swap: L-BTC -> BTC (25000 sats) to match screenshot
        let quote = QuoteBuilder::new_send(25000, load_test_pairs())
            .send(SwapAsset::Liquid)
            .receive(SwapAsset::Onchain)
            .build()
            .unwrap();

        // From swap-chain.json L-BTC -> BTC pair:
        // percentage: 0.1, server: 481, user.claim: 333, user.lockup: 27
        // boltz_fee = ceil(0.1 / 100 * 25000) = ceil(25) = 25
        // network_fee = server + claim = 481 + 333 = 814
        // receive_amount = 25000 - 25 - 814 = 24161
        // This matches the screenshot exactly!
        assert_eq!(quote.send_amount, 25000);
        assert_eq!(quote.boltz_fee, 25);
        assert_eq!(quote.network_fee, 814);
        assert_eq!(quote.receive_amount, 24161);
        assert_eq!(quote.min, 25000);
        assert_eq!(quote.max, 25_000_000);
    }

    #[test]
    fn test_quote_builder_chain_btc_to_lbtc() {
        // Test chain swap: BTC -> L-BTC (50000 sats)
        let quote = QuoteBuilder::new_send(50000, load_test_pairs())
            .send(SwapAsset::Onchain)
            .receive(SwapAsset::Liquid)
            .build()
            .unwrap();

        // From swap-chain.json BTC -> L-BTC pair:
        // percentage: 0.1, server: 480, user.claim: 20, user.lockup: 462
        // boltz_fee = ceil(0.1 / 100 * 50000) = ceil(50) = 50
        // network_fee = server + claim + LIQUID_UNCOOPERATIVE_EXTRA = 480 + 20 + 3 = 503
        // receive_amount = 50000 - 50 - 503 = 49447
        assert_eq!(quote.send_amount, 50000);
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
        let result = QuoteBuilder::new_send(25000, swap_info.clone())
            .send(SwapAsset::Lightning)
            .receive(SwapAsset::Lightning)
            .build();

        assert!(matches!(result, Err(Error::InvalidSwapPair { .. })));

        // Test invalid pair: Onchain -> Onchain
        let result = QuoteBuilder::new_send(25000, swap_info)
            .send(SwapAsset::Onchain)
            .receive(SwapAsset::Onchain)
            .build();

        assert!(matches!(result, Err(Error::InvalidSwapPair { .. })));
    }

    #[test]
    fn test_quote_builder_missing_params() {
        // Test missing 'send' param
        let swap_info = load_test_pairs();
        let result = QuoteBuilder::new_send(25000, swap_info.clone())
            .receive(SwapAsset::Liquid)
            .build();

        assert!(matches!(result, Err(Error::MissingQuoteParam("send"))));

        // Test missing 'receive' param
        let result = QuoteBuilder::new_send(25000, swap_info)
            .send(SwapAsset::Lightning)
            .build();

        assert!(matches!(result, Err(Error::MissingQuoteParam("receive"))));
    }

    // === Quote by receive amount tests ===

    #[test]
    fn test_quote_receive_reverse() {
        // Inverse of test_quote_builder_reverse
        // quote(25000) -> receive_amount = 24887
        // quote_receive(24887) -> send_amount should be 25000
        let quote = QuoteBuilder::new_receive(24887, load_test_pairs())
            .send(SwapAsset::Lightning)
            .receive(SwapAsset::Liquid)
            .build()
            .unwrap();

        assert_eq!(quote.send_amount, 25000);
        assert_eq!(quote.receive_amount, 24887);
        assert_eq!(quote.network_fee, 50);
        assert_eq!(quote.boltz_fee, 63);
    }

    #[test]
    fn test_quote_receive_submarine() {
        // Inverse of test_quote_builder_submarine
        // quote(25000) -> receive_amount = 24956
        // quote_receive(24956) -> send_amount should be 25000
        let quote = QuoteBuilder::new_receive(24956, load_test_pairs())
            .send(SwapAsset::Liquid)
            .receive(SwapAsset::Lightning)
            .build()
            .unwrap();

        assert_eq!(quote.send_amount, 25000);
        assert_eq!(quote.receive_amount, 24956);
        assert_eq!(quote.network_fee, 19);
        assert_eq!(quote.boltz_fee, 25);
    }

    #[test]
    fn test_quote_receive_chain_lbtc_to_btc() {
        // Inverse of test_quote_builder_chain_lbtc_to_btc
        // quote(25000) -> receive_amount = 24161
        // quote_receive(24161) -> send_amount should be 25000
        let quote = QuoteBuilder::new_receive(24161, load_test_pairs())
            .send(SwapAsset::Liquid)
            .receive(SwapAsset::Onchain)
            .build()
            .unwrap();

        assert_eq!(quote.send_amount, 25000);
        assert_eq!(quote.receive_amount, 24161);
        assert_eq!(quote.network_fee, 814);
        assert_eq!(quote.boltz_fee, 25);
    }

    #[test]
    fn test_quote_receive_chain_btc_to_lbtc() {
        // Inverse of test_quote_builder_chain_btc_to_lbtc
        // quote(50000) -> receive_amount = 49447
        // quote_receive(49447) -> send_amount should be 50000
        let quote = QuoteBuilder::new_receive(49447, load_test_pairs())
            .send(SwapAsset::Onchain)
            .receive(SwapAsset::Liquid)
            .build()
            .unwrap();

        assert_eq!(quote.send_amount, 50000);
        assert_eq!(quote.receive_amount, 49447);
        assert_eq!(quote.network_fee, 503);
        assert_eq!(quote.boltz_fee, 50);
    }

    #[test]
    fn test_quote_coherence_reverse() {
        // Test that quote and quote_receive are coherent for reverse swaps
        let swap_info = load_test_pairs();

        for send in [1000u64, 5000, 10000, 25000, 50000, 100000, 1000000] {
            let forward_quote = QuoteBuilder::new_send(send, swap_info.clone())
                .send(SwapAsset::Lightning)
                .receive(SwapAsset::Liquid)
                .build()
                .unwrap();

            let inverse_quote =
                QuoteBuilder::new_receive(forward_quote.receive_amount, swap_info.clone())
                    .send(SwapAsset::Lightning)
                    .receive(SwapAsset::Liquid)
                    .build()
                    .unwrap();

            assert_eq!(
                inverse_quote.send_amount, send,
                "Coherence failed for send={}: forward.receive={}, inverse.send={}",
                send, forward_quote.receive_amount, inverse_quote.send_amount
            );
        }
    }

    #[test]
    fn test_quote_coherence_submarine() {
        // Test that quote and quote_receive are coherent for submarine swaps
        let swap_info = load_test_pairs();

        for send in [1000u64, 5000, 10000, 25000, 50000, 100000, 1000000] {
            let forward_quote = QuoteBuilder::new_send(send, swap_info.clone())
                .send(SwapAsset::Liquid)
                .receive(SwapAsset::Lightning)
                .build()
                .unwrap();

            let inverse_quote =
                QuoteBuilder::new_receive(forward_quote.receive_amount, swap_info.clone())
                    .send(SwapAsset::Liquid)
                    .receive(SwapAsset::Lightning)
                    .build()
                    .unwrap();

            assert_eq!(
                inverse_quote.send_amount, send,
                "Coherence failed for send={}: forward.receive={}, inverse.send={}",
                send, forward_quote.receive_amount, inverse_quote.send_amount
            );
        }
    }

    #[test]
    fn test_quote_coherence_chain() {
        // Test that quote and quote_receive are coherent for chain swaps
        let swap_info = load_test_pairs();

        // L-BTC -> BTC
        for send in [25000u64, 50000, 100000, 1000000] {
            let forward_quote = QuoteBuilder::new_send(send, swap_info.clone())
                .send(SwapAsset::Liquid)
                .receive(SwapAsset::Onchain)
                .build()
                .unwrap();

            let inverse_quote =
                QuoteBuilder::new_receive(forward_quote.receive_amount, swap_info.clone())
                    .send(SwapAsset::Liquid)
                    .receive(SwapAsset::Onchain)
                    .build()
                    .unwrap();

            assert_eq!(
                inverse_quote.send_amount, send,
                "L-BTC->BTC coherence failed for send={}: forward.receive={}, inverse.send={}",
                send, forward_quote.receive_amount, inverse_quote.send_amount
            );
        }

        // BTC -> L-BTC
        for send in [25000u64, 50000, 100000, 1000000] {
            let forward_quote = QuoteBuilder::new_send(send, swap_info.clone())
                .send(SwapAsset::Onchain)
                .receive(SwapAsset::Liquid)
                .build()
                .unwrap();

            let inverse_quote =
                QuoteBuilder::new_receive(forward_quote.receive_amount, swap_info.clone())
                    .send(SwapAsset::Onchain)
                    .receive(SwapAsset::Liquid)
                    .build()
                    .unwrap();

            assert_eq!(
                inverse_quote.send_amount, send,
                "BTC->L-BTC coherence failed for send={}: forward.receive={}, inverse.send={}",
                send, forward_quote.receive_amount, inverse_quote.send_amount
            );
        }
    }

    /// Test to analyze the behavior of the ceiling approximation in calculate_send_amount.
    ///
    /// Key insight: Due to ceiling in the boltz_fee formula, multiple send values can
    /// produce the same receive value. For example:
    /// - send=400: boltz_fee=ceil(0.25*400/100)=1, receive=400-1-50=349
    /// - send=401: boltz_fee=ceil(0.25*401/100)=2, receive=401-2-50=349
    ///
    /// Both give receive=349! So `calculate_send_amount(349)` correctly returns 400
    /// (the minimum valid send), not 401.
    ///
    /// The true invariant is: `forward(inverse(r)) == r`, NOT `inverse(forward(s)) == s`
    #[test]
    fn test_calculate_send_amount_exhaustive() {
        // Test with various percentage values used by Boltz
        let test_cases: &[(f64, u64, &str)] = &[
            (0.25, 50, "reverse (0.25%)"),
            (0.1, 19, "submarine (0.1%)"),
            (0.1, 814, "chain L-BTC->BTC"),
            (0.1, 503, "chain BTC->L-BTC"),
            (0.5, 100, "hypothetical 0.5%"),
            (1.0, 50, "hypothetical 1%"),
        ];

        for (percentage, network_fee, description) in test_cases {
            let mut overshot_count = 0u64;
            let mut undershot_count = 0u64;
            let mut exact_count = 0u64;

            // Test a wide range of receive amounts directly
            for receive in 1u64..100_000 {
                // Calculate the minimum send amount for this receive
                let base = receive + network_fee;
                let rate = 1.0 - percentage / 100.0;
                let send_approx = (base as f64 / rate).ceil() as u64;

                // Verify what receive we'd actually get with send_approx
                let bf_check = ((*percentage * send_approx as f64) / 100.0).ceil() as u64;
                let calculated_receive = send_approx.saturating_sub(bf_check + network_fee);

                match calculated_receive.cmp(&receive) {
                    std::cmp::Ordering::Equal => exact_count += 1,
                    std::cmp::Ordering::Greater => overshot_count += 1,
                    std::cmp::Ordering::Less => undershot_count += 1,
                }
            }

            assert!(
                exact_count > 0,
                "{}: unexpectedly had no exact cases - this would be a bug!",
                description
            );

            // With the ceiling formula, we should never undershoot
            // Overshooting is acceptable (we get slightly more than requested)
            assert_eq!(
                undershot_count, 0,
                "{}: unexpectedly had {} undershot cases - this would be a bug!",
                description, undershot_count
            );

            assert_eq!(
                overshot_count, 0,
                "{}: unexpectedly had {} overshot cases - this would be a bug!",
                description, overshot_count
            );
        }
    }

    /// Test that verifies the true coherence invariant: forward(inverse(r)) == r
    ///
    /// This means: if you want to receive `r` sats, and we tell you to send `s` sats,
    /// then sending `s` sats should give you exactly `r` sats (not less).
    #[test]
    fn test_calculate_send_amount_forward_inverse_coherence() {
        let percentage = 0.25;
        let network_fee = 50;

        for receive in 1u64..100_000 {
            let send = calculate_send_amount(receive, network_fee, percentage);
            let boltz_fee = ((percentage * send as f64) / 100.0).ceil() as u64;
            let actual_receive = send.saturating_sub(boltz_fee + network_fee);

            // The key invariant: sending `send` must give at least `receive`
            assert!(
                actual_receive >= receive,
                "CRITICAL: send={} gives receive={}, but wanted at least {}",
                send,
                actual_receive,
                receive
            );

            // Ideally they're equal (minimum send for desired receive)
            // But due to ceiling, we might get slightly more
            if actual_receive != receive {
                // This is OK - it means send is the minimum that achieves >= receive
                // Verify that send-1 would give less than receive
                if send > 0 {
                    let bf_minus1 = ((percentage * (send - 1) as f64) / 100.0).ceil() as u64;
                    let recv_minus1 = (send - 1).saturating_sub(bf_minus1 + network_fee);
                    assert!(
                        recv_minus1 < receive,
                        "send-1={} gives {}, which is >= {}, so send={} is not minimal",
                        send - 1,
                        recv_minus1,
                        receive,
                        send
                    );
                }
            }
        }
    }
}
