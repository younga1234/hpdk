/// Order Book Analysis Module
///
/// Based on academic research:
/// - "Exploring Microstructural Dynamics in Cryptocurrency Limit Order Books" (arXiv 2506.05764v1)
/// - Market Microstructure Theory
/// - Kyle's Lambda, VPIN, Order Imbalance

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Order book snapshot
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderBookSnapshot {
    pub timestamp: u64,
    pub bids: Vec<PriceLevel>,  // Buy orders (descending price)
    pub asks: Vec<PriceLevel>,  // Sell orders (ascending price)
}

/// Price level in order book
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PriceLevel {
    pub price: f64,
    pub volume: f64,
}

/// Order Book Analysis Results
#[derive(Debug, Clone)]
pub struct OrderBookSignals {
    pub oir: f64,                    // Order Imbalance Ratio
    pub wpp_bid: f64,                // Weighted Price Pressure (Bid)
    pub wpp_ask: f64,                // Weighted Price Pressure (Ask)
    pub spread: f64,                 // Bid-ask spread
    pub mid_price: f64,              // Mid price
    pub signal: OrderBookSignal,     // Trading signal
    pub kyle_lambda: Option<f64>,    // Price impact (requires historical data)
    pub vpin: Option<f64>,           // Volume-synchronized PIN
}

#[derive(Debug, Clone, PartialEq)]
pub enum OrderBookSignal {
    StrongBuy,
    Buy,
    Neutral,
    Sell,
    StrongSell,
}

/// Order Book Analyzer
pub struct OrderBookAnalyzer {
    depth_levels: usize,  // Number of levels to analyze
    oir_threshold: f64,   // OIR threshold for signals
}

impl OrderBookAnalyzer {
    pub fn new(depth_levels: usize) -> Self {
        Self {
            depth_levels,
            oir_threshold: 0.3, // Research-based threshold
        }
    }

    /// Analyze order book and generate signals
    pub fn analyze(&self, snapshot: &OrderBookSnapshot) -> OrderBookSignals {
        let mid_price = self.calculate_mid_price(snapshot);
        let spread = self.calculate_spread(snapshot);
        let oir = self.calculate_order_imbalance_ratio(snapshot);
        let (wpp_bid, wpp_ask) = self.calculate_weighted_price_pressure(snapshot, mid_price);

        let signal = self.generate_signal(oir, wpp_bid, wpp_ask);

        OrderBookSignals {
            oir,
            wpp_bid,
            wpp_ask,
            spread,
            mid_price,
            signal,
            kyle_lambda: None, // Requires historical price data
            vpin: None,        // Requires trade flow data
        }
    }

    /// Calculate Order Imbalance Ratio (OIR)
    ///
    /// OIR = (Bid Volume - Ask Volume) / (Bid Volume + Ask Volume)
    ///
    /// OIR > 0.3: Strong buying pressure
    /// OIR < -0.3: Strong selling pressure
    fn calculate_order_imbalance_ratio(&self, snapshot: &OrderBookSnapshot) -> f64 {
        let bid_volume: f64 = snapshot.bids
            .iter()
            .take(self.depth_levels)
            .map(|level| level.volume)
            .sum();

        let ask_volume: f64 = snapshot.asks
            .iter()
            .take(self.depth_levels)
            .map(|level| level.volume)
            .sum();

        let total_volume = bid_volume + ask_volume;

        if total_volume == 0.0 {
            return 0.0;
        }

        (bid_volume - ask_volume) / total_volume
    }

    /// Calculate Weighted Price Pressure (WPP)
    ///
    /// WPP = Σ(Volume_i × 1/Distance_i) for top N levels
    ///
    /// Higher WPP on bid side: Buying pressure
    /// Higher WPP on ask side: Selling pressure
    fn calculate_weighted_price_pressure(&self, snapshot: &OrderBookSnapshot, mid_price: f64) -> (f64, f64) {
        let wpp_bid: f64 = snapshot.bids
            .iter()
            .take(self.depth_levels)
            .map(|level| {
                let distance = (mid_price - level.price).abs() + 0.0001; // Avoid division by zero
                level.volume / distance
            })
            .sum();

        let wpp_ask: f64 = snapshot.asks
            .iter()
            .take(self.depth_levels)
            .map(|level| {
                let distance = (level.price - mid_price).abs() + 0.0001;
                level.volume / distance
            })
            .sum();

        (wpp_bid, wpp_ask)
    }

    /// Calculate mid price
    fn calculate_mid_price(&self, snapshot: &OrderBookSnapshot) -> f64 {
        if snapshot.bids.is_empty() || snapshot.asks.is_empty() {
            return 0.0;
        }

        let best_bid = snapshot.bids[0].price;
        let best_ask = snapshot.asks[0].price;

        (best_bid + best_ask) / 2.0
    }

    /// Calculate bid-ask spread
    fn calculate_spread(&self, snapshot: &OrderBookSnapshot) -> f64 {
        if snapshot.bids.is_empty() || snapshot.asks.is_empty() {
            return 0.0;
        }

        let best_bid = snapshot.bids[0].price;
        let best_ask = snapshot.asks[0].price;

        best_ask - best_bid
    }

    /// Generate trading signal from order book metrics
    fn generate_signal(&self, oir: f64, wpp_bid: f64, wpp_ask: f64) -> OrderBookSignal {
        let wpp_ratio = if wpp_ask > 0.0 {
            wpp_bid / wpp_ask
        } else {
            1.0
        };

        // Strong buy: High positive OIR + Strong bid pressure
        if oir > 0.4 && wpp_ratio > 1.5 {
            OrderBookSignal::StrongBuy
        }
        // Buy: Positive OIR + Bid pressure
        else if oir > self.oir_threshold && wpp_ratio > 1.2 {
            OrderBookSignal::Buy
        }
        // Strong sell: High negative OIR + Strong ask pressure
        else if oir < -0.4 && wpp_ratio < 0.67 {
            OrderBookSignal::StrongSell
        }
        // Sell: Negative OIR + Ask pressure
        else if oir < -self.oir_threshold && wpp_ratio < 0.83 {
            OrderBookSignal::Sell
        }
        // Neutral
        else {
            OrderBookSignal::Neutral
        }
    }

    /// Calculate Kyle's Lambda (Price Impact)
    ///
    /// λ = ΔPrice / ΔVolume
    ///
    /// Requires historical price and volume data
    /// High λ: Low liquidity, high slippage risk
    pub fn calculate_kyle_lambda(&self, price_changes: &[f64], volume_changes: &[f64]) -> Option<f64> {
        if price_changes.len() != volume_changes.len() || price_changes.is_empty() {
            return None;
        }

        let sum_price: f64 = price_changes.iter().sum();
        let sum_volume: f64 = volume_changes.iter().sum();

        if sum_volume.abs() < 0.0001 {
            return None;
        }

        Some(sum_price.abs() / sum_volume)
    }

    /// Calculate VPIN (Volume-Synchronized Probability of Informed Trading)
    ///
    /// VPIN = |Buy Volume - Sell Volume| / Total Volume
    ///
    /// High VPIN (> 0.8): Toxic flow, informed traders active
    /// Low VPIN (< 0.3): Safe liquidity
    pub fn calculate_vpin(&self, buy_volume: f64, sell_volume: f64) -> f64 {
        let total_volume = buy_volume + sell_volume;

        if total_volume == 0.0 {
            return 0.0;
        }

        (buy_volume - sell_volume).abs() / total_volume
    }

    /// Calculate Amihud Illiquidity Measure
    ///
    /// Illiquidity = |Return| / Volume
    ///
    /// Higher value = Less liquid market
    pub fn calculate_amihud_illiquidity(&self, price_return: f64, volume: f64) -> f64 {
        if volume == 0.0 {
            return f64::MAX;
        }

        price_return.abs() / volume
    }
}

/// Order Book Manager
/// Maintains and updates order book state
pub struct OrderBookManager {
    order_books: HashMap<String, OrderBookSnapshot>,
    analyzer: OrderBookAnalyzer,
}

impl OrderBookManager {
    pub fn new(depth_levels: usize) -> Self {
        Self {
            order_books: HashMap::new(),
            analyzer: OrderBookAnalyzer::new(depth_levels),
        }
    }

    /// Update order book for a ticker
    pub fn update(&mut self, ticker: String, snapshot: OrderBookSnapshot) {
        self.order_books.insert(ticker, snapshot);
    }

    /// Get analysis for a ticker
    pub fn analyze(&self, ticker: &str) -> Option<OrderBookSignals> {
        self.order_books
            .get(ticker)
            .map(|snapshot| self.analyzer.analyze(snapshot))
    }

    /// Check if order book is available
    pub fn has_orderbook(&self, ticker: &str) -> bool {
        self.order_books.contains_key(ticker)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_orderbook() -> OrderBookSnapshot {
        OrderBookSnapshot {
            timestamp: 1234567890,
            bids: vec![
                PriceLevel { price: 50000.0, volume: 1.5 },
                PriceLevel { price: 49990.0, volume: 2.0 },
                PriceLevel { price: 49980.0, volume: 1.0 },
            ],
            asks: vec![
                PriceLevel { price: 50010.0, volume: 1.0 },
                PriceLevel { price: 50020.0, volume: 1.5 },
                PriceLevel { price: 50030.0, volume: 2.0 },
            ],
        }
    }

    #[test]
    fn test_order_imbalance_ratio() {
        let analyzer = OrderBookAnalyzer::new(3);
        let orderbook = create_test_orderbook();

        let oir = analyzer.calculate_order_imbalance_ratio(&orderbook);

        // Bid volume: 4.5, Ask volume: 4.5
        // OIR = (4.5 - 4.5) / (4.5 + 4.5) = 0.0
        assert!((oir - 0.0).abs() < 0.01);
    }

    #[test]
    fn test_mid_price() {
        let analyzer = OrderBookAnalyzer::new(3);
        let orderbook = create_test_orderbook();

        let mid_price = analyzer.calculate_mid_price(&orderbook);

        // Mid = (50000 + 50010) / 2 = 50005
        assert!((mid_price - 50005.0).abs() < 0.01);
    }

    #[test]
    fn test_spread() {
        let analyzer = OrderBookAnalyzer::new(3);
        let orderbook = create_test_orderbook();

        let spread = analyzer.calculate_spread(&orderbook);

        // Spread = 50010 - 50000 = 10
        assert!((spread - 10.0).abs() < 0.01);
    }

    #[test]
    fn test_vpin() {
        let analyzer = OrderBookAnalyzer::new(3);

        let vpin = analyzer.calculate_vpin(100.0, 80.0);

        // VPIN = |100 - 80| / (100 + 80) = 20 / 180 = 0.111
        assert!((vpin - 0.111).abs() < 0.01);
    }
}
