use crate::websocket::Candle;

/// Pump & Dump 감지기
///
/// 학술 논문 기반 알고리즘:
/// - 거래량 400% 증가 + 가격 90% 상승
/// - EWMA (Exponential Weighted Moving Average) 사용
/// - 참고: "Detecting Crypto Pump-and-Dump Schemes" (arXiv)
pub struct PumpDetector {
    /// EWMA 윈도우 (시간 단위)
    ewma_window: usize,
    /// 거래량 증가 임계값 (배수)
    volume_threshold: f64,
    /// 가격 증가 임계값 (%)
    price_threshold: f64,
}

impl PumpDetector {
    pub fn new() -> Self {
        Self {
            ewma_window: 12, // 12시간 (5분봉 기준: 12*12 = 144개 캔들)
            volume_threshold: 4.0, // 400% (4배)
            price_threshold: 90.0, // 90%
        }
    }

    /// 커스텀 임계값 설정
    pub fn with_thresholds(volume_threshold: f64, price_threshold: f64) -> Self {
        Self {
            ewma_window: 12,
            volume_threshold,
            price_threshold,
        }
    }

    /// Pump 신호 감지
    pub fn detect_pump(&self, candles: &[Candle]) -> (bool, Option<String>) {
        if candles.len() < self.ewma_window + 1 {
            return (false, None);
        }

        // EWMA 계산
        let ewma_volume = self.calculate_ewma(
            &candles[..candles.len() - 1]
                .iter()
                .map(|c| c.acc_trade_price)
                .collect::<Vec<_>>(),
            self.ewma_window,
        );

        let ewma_price = self.calculate_ewma(
            &candles[..candles.len() - 1]
                .iter()
                .map(|c| c.close)
                .collect::<Vec<_>>(),
            self.ewma_window,
        );

        let current_candle = candles.last().unwrap();
        let current_volume = current_candle.acc_trade_price;
        let current_price = current_candle.close;

        // 거래량 증가율
        let volume_ratio = if ewma_volume > 0.0 {
            current_volume / ewma_volume
        } else {
            1.0
        };

        // 가격 증가율
        let price_change = if ewma_price > 0.0 {
            ((current_price - ewma_price) / ewma_price) * 100.0
        } else {
            0.0
        };

        // Pump 감지 조건
        let is_pump = volume_ratio >= self.volume_threshold
            && price_change >= self.price_threshold;

        if is_pump {
            let reason = format!(
                "🚨 PUMP 감지! 거래량: {:.1}배 (기준: {:.1}배), 가격: {:.1}% (기준: {:.1}%)",
                volume_ratio,
                self.volume_threshold,
                price_change,
                self.price_threshold
            );
            (true, Some(reason))
        } else {
            (false, None)
        }
    }

    /// 급등 초기 단계 감지 (Pump 전 단계)
    ///
    /// 더 낮은 임계값으로 조기 진입 가능
    pub fn detect_surge(&self, candles: &[Candle]) -> (bool, Option<String>) {
        if candles.len() < 10 {
            return (false, None);
        }

        // 최근 5개 캔들 평균
        let recent_avg_volume: f64 = candles[candles.len() - 5..]
            .iter()
            .map(|c| c.acc_trade_price)
            .sum::<f64>()
            / 5.0;

        // 이전 20개 캔들 평균
        let base_avg_volume: f64 = if candles.len() >= 25 {
            candles[candles.len() - 25..candles.len() - 5]
                .iter()
                .map(|c| c.acc_trade_price)
                .sum::<f64>()
                / 20.0
        } else {
            recent_avg_volume
        };

        // 급등 조기 신호: 거래량 2배 이상 증가
        let surge_ratio = if base_avg_volume > 0.0 {
            recent_avg_volume / base_avg_volume
        } else {
            1.0
        };

        let current_candle = candles.last().unwrap();
        let price_change = ((current_candle.close - current_candle.open) / current_candle.open) * 100.0;

        // 조기 진입 조건: 거래량 2배 + 가격 상승
        let is_surge = surge_ratio >= 2.0 && price_change > 0.5;

        if is_surge {
            let reason = format!(
                "⚡ 급등 조기 신호! 거래량: {:.2}배, 가격 변화: {:.2}%",
                surge_ratio,
                price_change
            );
            (true, Some(reason))
        } else {
            (false, None)
        }
    }

    /// EWMA 계산 (지수 가중 이동평균)
    fn calculate_ewma(&self, data: &[f64], window: usize) -> f64 {
        if data.is_empty() {
            return 0.0;
        }

        let alpha = 2.0 / (window as f64 + 1.0);
        let mut ewma = data[0];

        for &value in data.iter().skip(1) {
            ewma = alpha * value + (1.0 - alpha) * ewma;
        }

        ewma
    }

    /// 변동성 체크 (비정상적인 변동 감지)
    pub fn check_abnormal_volatility(&self, candles: &[Candle]) -> bool {
        if candles.len() < 20 {
            return false;
        }

        // 최근 10개 캔들의 가격 변동폭
        let recent_volatility: f64 = candles[candles.len() - 10..]
            .windows(2)
            .map(|w| {
                let change = ((w[1].close - w[0].close) / w[0].close).abs();
                change
            })
            .sum::<f64>()
            / 9.0;

        // 이전 10개 캔들의 가격 변동폭
        let base_volatility: f64 = candles[candles.len() - 20..candles.len() - 10]
            .windows(2)
            .map(|w| {
                let change = ((w[1].close - w[0].close) / w[0].close).abs();
                change
            })
            .sum::<f64>()
            / 9.0;

        // 변동성이 3배 이상 증가하면 비정상
        if base_volatility > 0.0 {
            recent_volatility / base_volatility >= 3.0
        } else {
            false
        }
    }
}

impl Default for PumpDetector {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn create_test_candles(count: usize) -> Vec<Candle> {
        (0..count)
            .map(|i| Candle {
                ticker: "KRW-BTC".to_string(),
                start_time: Utc::now(),
                open: 50000.0,
                high: 51000.0,
                low: 49000.0,
                close: 50000.0 + (i as f64 * 100.0),
                volume: 100.0,
                acc_trade_price: 1000000.0,
                tick_count: 100,
            })
            .collect()
    }

    #[test]
    fn test_ewma_calculation() {
        let detector = PumpDetector::new();
        let data = vec![100.0, 110.0, 105.0, 115.0, 120.0];
        let ewma = detector.calculate_ewma(&data, 3);
        assert!(ewma > 0.0);
    }

    #[test]
    fn test_no_pump_with_normal_data() {
        let detector = PumpDetector::new();
        let candles = create_test_candles(20);
        let (is_pump, _) = detector.detect_pump(&candles);
        assert!(!is_pump);
    }
}
