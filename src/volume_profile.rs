/// Volume Profile & VWAP Module
///
/// Based on professional trading strategies:
/// - Point of Control (POC): Price level with highest volume
/// - Value Area (VA): 70% of volume concentration
/// - VWAP: Volume-Weighted Average Price
/// - Anchored VWAP: From specific events

use crate::websocket::Candle;
use chrono::{DateTime, Utc};
use std::collections::HashMap;

/// Volume Profile Data
#[derive(Debug, Clone)]
pub struct VolumeProfile {
    pub price_levels: Vec<VolumePriceLevel>,
    pub poc: f64,              // Point of Control
    pub value_area_high: f64,  // Value Area High (70% volume)
    pub value_area_low: f64,   // Value Area Low
    pub total_volume: f64,
    pub period_high: f64,
    pub period_low: f64,
}

#[derive(Debug, Clone)]
pub struct VolumePriceLevel {
    pub price: f64,
    pub volume: f64,
    pub percentage: f64,  // % of total volume
}

/// VWAP Data
#[derive(Debug, Clone)]
pub struct VWAPData {
    pub vwap: f64,
    pub upper_band: f64,  // VWAP + stddev
    pub lower_band: f64,  // VWAP - stddev
    pub anchor_time: DateTime<Utc>,
}

/// Volume Profile Analyzer
pub struct VolumeProfileAnalyzer {
    tick_size: f64,  // Price resolution for binning
}

impl VolumeProfileAnalyzer {
    pub fn new(tick_size: f64) -> Self {
        Self { tick_size }
    }

    /// Build volume profile from candles
    pub fn build_profile(&self, candles: &[Candle]) -> VolumeProfile {
        if candles.is_empty() {
            return VolumeProfile {
                price_levels: vec![],
                poc: 0.0,
                value_area_high: 0.0,
                value_area_low: 0.0,
                total_volume: 0.0,
                period_high: 0.0,
                period_low: 0.0,
            };
        }

        // Find price range
        let mut period_high = candles[0].high;
        let mut period_low = candles[0].low;

        for candle in candles {
            period_high = period_high.max(candle.high);
            period_low = period_low.min(candle.low);
        }

        // Create price bins
        let num_bins = ((period_high - period_low) / self.tick_size).ceil() as usize + 1;
        let mut volume_bins: HashMap<usize, f64> = HashMap::new();

        // Distribute volume across price levels
        for candle in candles {
            // Simplified: Assume uniform distribution within candle
            let candle_range = candle.high - candle.low;
            if candle_range == 0.0 {
                // No range, assign all volume to close price
                let bin = self.price_to_bin(candle.close, period_low);
                *volume_bins.entry(bin).or_insert(0.0) += candle.volume;
            } else {
                // Distribute volume across range
                let volume_per_tick = candle.volume / (candle_range / self.tick_size);
                let start_bin = self.price_to_bin(candle.low, period_low);
                let end_bin = self.price_to_bin(candle.high, period_low);

                for bin in start_bin..=end_bin.min(num_bins - 1) {
                    *volume_bins.entry(bin).or_insert(0.0) += volume_per_tick;
                }
            }
        }

        // Convert bins to price levels
        let total_volume: f64 = volume_bins.values().sum();

        let mut price_levels: Vec<VolumePriceLevel> = volume_bins
            .into_iter()
            .map(|(bin, volume)| {
                let price = self.bin_to_price(bin, period_low);
                let percentage = if total_volume > 0.0 {
                    (volume / total_volume) * 100.0
                } else {
                    0.0
                };
                VolumePriceLevel {
                    price,
                    volume,
                    percentage,
                }
            })
            .collect();

        // Sort by volume (descending)
        price_levels.sort_by(|a, b| b.volume.partial_cmp(&a.volume).unwrap());

        // Find POC (highest volume level)
        let poc = price_levels.first().map(|l| l.price).unwrap_or(0.0);

        // Calculate Value Area (70% of volume)
        let value_area_volume = total_volume * 0.7;
        let (vah, val) = self.calculate_value_area(&price_levels, value_area_volume);

        VolumeProfile {
            price_levels,
            poc,
            value_area_high: vah,
            value_area_low: val,
            total_volume,
            period_high,
            period_low,
        }
    }

    /// Calculate Value Area (70% volume concentration)
    fn calculate_value_area(&self, price_levels: &[VolumePriceLevel], target_volume: f64) -> (f64, f64) {
        if price_levels.is_empty() {
            return (0.0, 0.0);
        }

        let mut accumulated_volume = 0.0;
        let mut va_prices: Vec<f64> = vec![];

        for level in price_levels {
            accumulated_volume += level.volume;
            va_prices.push(level.price);

            if accumulated_volume >= target_volume {
                break;
            }
        }

        if va_prices.is_empty() {
            return (0.0, 0.0);
        }

        let vah = va_prices.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        let val = va_prices.iter().cloned().fold(f64::INFINITY, f64::min);

        (vah, val)
    }

    fn price_to_bin(&self, price: f64, base_price: f64) -> usize {
        ((price - base_price) / self.tick_size).floor() as usize
    }

    fn bin_to_price(&self, bin: usize, base_price: f64) -> f64 {
        base_price + (bin as f64 * self.tick_size)
    }
}

/// VWAP Calculator
pub struct VWAPCalculator {
    anchor_time: DateTime<Utc>,
}

impl VWAPCalculator {
    pub fn new(anchor_time: DateTime<Utc>) -> Self {
        Self { anchor_time }
    }

    /// Calculate VWAP from candles
    ///
    /// VWAP = Σ(Price × Volume) / Σ(Volume)
    ///
    /// Typically uses typical price: (High + Low + Close) / 3
    pub fn calculate(&self, candles: &[Candle]) -> Option<VWAPData> {
        if candles.is_empty() {
            return None;
        }

        // Filter candles after anchor time
        let filtered: Vec<&Candle> = candles
            .iter()
            .filter(|c| c.start_time >= self.anchor_time)
            .collect();

        if filtered.is_empty() {
            return None;
        }

        let mut sum_pv = 0.0; // Price × Volume
        let mut sum_v = 0.0;  // Volume
        let mut price_values = vec![];

        for candle in filtered {
            let typical_price = (candle.high + candle.low + candle.close) / 3.0;
            let volume = candle.volume;

            sum_pv += typical_price * volume;
            sum_v += volume;
            price_values.push(typical_price);
        }

        if sum_v == 0.0 {
            return None;
        }

        let vwap = sum_pv / sum_v;

        // Calculate standard deviation
        let mean_price = price_values.iter().sum::<f64>() / price_values.len() as f64;
        let variance = price_values
            .iter()
            .map(|p| (p - mean_price).powi(2))
            .sum::<f64>()
            / price_values.len() as f64;
        let stddev = variance.sqrt();

        Some(VWAPData {
            vwap,
            upper_band: vwap + stddev,
            lower_band: vwap - stddev,
            anchor_time: self.anchor_time,
        })
    }

    /// Session VWAP (from session start)
    pub fn session_vwap(candles: &[Candle]) -> Option<VWAPData> {
        if let Some(first_candle) = candles.first() {
            let calculator = VWAPCalculator::new(first_candle.start_time);
            calculator.calculate(candles)
        } else {
            None
        }
    }

    /// Rolling VWAP (last N candles)
    pub fn rolling_vwap(candles: &[Candle], period: usize) -> Option<VWAPData> {
        if candles.len() < period {
            return None;
        }

        let start_candle = &candles[candles.len() - period];
        let calculator = VWAPCalculator::new(start_candle.start_time);
        calculator.calculate(candles)
    }
}

/// Volume Profile Signal Generator
pub struct VolumeProfileSignal;

impl VolumeProfileSignal {
    /// Generate trading signal from volume profile
    pub fn analyze(
        current_price: f64,
        profile: &VolumeProfile,
        vwap_data: Option<&VWAPData>,
    ) -> VolumeProfileSignalType {
        // POC acts as support/resistance
        let distance_from_poc = (current_price - profile.poc) / profile.poc;

        // Value Area analysis
        let in_value_area = current_price >= profile.value_area_low
            && current_price <= profile.value_area_high;

        // VWAP analysis
        let above_vwap = if let Some(vwap) = vwap_data {
            current_price > vwap.vwap
        } else {
            false
        };

        // Signal logic
        if current_price < profile.value_area_low && above_vwap {
            // Below VA but above VWAP: Potential bounce from support
            VolumeProfileSignalType::BuyOpportunity
        } else if current_price > profile.value_area_high && !above_vwap {
            // Above VA but below VWAP: Potential rejection
            VolumeProfileSignalType::SellOpportunity
        } else if distance_from_poc.abs() < 0.005 && in_value_area {
            // Near POC in VA: Consolidation
            VolumeProfileSignalType::Consolidation
        } else if current_price > profile.value_area_high && above_vwap {
            // Above VA and VWAP: Strong uptrend
            VolumeProfileSignalType::StrongBullish
        } else if current_price < profile.value_area_low && !above_vwap {
            // Below VA and VWAP: Strong downtrend
            VolumeProfileSignalType::StrongBearish
        } else {
            VolumeProfileSignalType::Neutral
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum VolumeProfileSignalType {
    StrongBullish,
    BuyOpportunity,
    Neutral,
    Consolidation,
    SellOpportunity,
    StrongBearish,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_candles() -> Vec<Candle> {
        vec![
            Candle {
                ticker: "TEST".to_string(),
                open: 100.0,
                high: 105.0,
                low: 99.0,
                close: 103.0,
                volume: 1000.0,
                acc_trade_price: 100000.0,
                start_time: Utc::now(),
                tick_count: 10,
            },
            Candle {
                ticker: "TEST".to_string(),
                open: 103.0,
                high: 107.0,
                low: 102.0,
                close: 106.0,
                volume: 1500.0,
                acc_trade_price: 150000.0,
                start_time: Utc::now(),
                tick_count: 15,
            },
        ]
    }

    #[test]
    fn test_volume_profile_creation() {
        let analyzer = VolumeProfileAnalyzer::new(1.0);
        let candles = create_test_candles();

        let profile = analyzer.build_profile(&candles);

        assert!(profile.total_volume > 0.0);
        assert!(profile.period_high >= profile.period_low);
        assert!(profile.poc > 0.0);
    }

    #[test]
    fn test_vwap_calculation() {
        let candles = create_test_candles();
        let vwap = VWAPCalculator::session_vwap(&candles);

        assert!(vwap.is_some());
        let vwap_data = vwap.unwrap();
        assert!(vwap_data.vwap > 0.0);
        assert!(vwap_data.upper_band > vwap_data.vwap);
        assert!(vwap_data.lower_band < vwap_data.vwap);
    }
}
