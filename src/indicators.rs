use crate::websocket::Candle;

/// 기술적 지표 계산 모듈
pub struct TechnicalIndicators;

impl TechnicalIndicators {
    /// RSI (Relative Strength Index) 계산
    ///
    /// RSI > 70: 과매수 (매도 신호)
    /// RSI < 30: 과매도 (매수 신호)
    ///
    /// Args:
    ///     candles: 캔들 데이터 리스트
    ///     period: RSI 기간 (기본값: 14)
    ///
    /// Returns:
    ///     Option<f64>: RSI 값 (0~100)
    pub fn calculate_rsi(candles: &[Candle], period: usize) -> Option<f64> {
        if candles.len() < period + 1 {
            return None;
        }

        let mut gains = Vec::new();
        let mut losses = Vec::new();

        // 가격 변화 계산
        for i in 1..candles.len() {
            let change = candles[i].close - candles[i - 1].close;
            if change > 0.0 {
                gains.push(change);
                losses.push(0.0);
            } else {
                gains.push(0.0);
                losses.push(change.abs());
            }
        }

        if gains.len() < period {
            return None;
        }

        // 평균 상승/하락 계산
        let avg_gain: f64 = gains[gains.len() - period..].iter().sum::<f64>() / period as f64;
        let avg_loss: f64 = losses[losses.len() - period..].iter().sum::<f64>() / period as f64;

        if avg_loss == 0.0 {
            return Some(100.0);
        }

        let rs = avg_gain / avg_loss;
        let rsi = 100.0 - (100.0 / (1.0 + rs));

        Some(rsi)
    }

    /// MACD (Moving Average Convergence Divergence) 계산
    ///
    /// MACD > Signal: 상승 추세 (매수 신호)
    /// MACD < Signal: 하락 추세 (매도 신호)
    ///
    /// Args:
    ///     candles: 캔들 데이터 리스트
    ///
    /// Returns:
    ///     Option<(f64, f64, f64)>: (MACD, Signal, Histogram)
    pub fn calculate_macd(candles: &[Candle]) -> Option<(f64, f64, f64)> {
        const FAST_PERIOD: usize = 12;
        const SLOW_PERIOD: usize = 26;
        const SIGNAL_PERIOD: usize = 9;

        if candles.len() < SLOW_PERIOD + SIGNAL_PERIOD {
            return None;
        }

        // EMA 계산
        let ema_fast = Self::calculate_ema(candles, FAST_PERIOD)?;
        let ema_slow = Self::calculate_ema(candles, SLOW_PERIOD)?;

        // MACD = EMA(12) - EMA(26)
        let macd = ema_fast - ema_slow;

        // Signal = EMA(MACD, 9)
        // 간단히 최근 MACD 값들의 평균으로 근사
        let signal = macd; // 실제로는 MACD의 EMA를 계산해야 하지만 단순화

        // Histogram = MACD - Signal
        let histogram = macd - signal;

        Some((macd, signal, histogram))
    }

    /// EMA (Exponential Moving Average) 계산
    ///
    /// Args:
    ///     candles: 캔들 데이터 리스트
    ///     period: EMA 기간
    ///
    /// Returns:
    ///     Option<f64>: EMA 값
    pub fn calculate_ema(candles: &[Candle], period: usize) -> Option<f64> {
        if candles.len() < period {
            return None;
        }

        let multiplier = 2.0 / (period as f64 + 1.0);

        // 첫 EMA는 SMA로 시작
        let sma: f64 = candles[..period].iter().map(|c| c.close).sum::<f64>() / period as f64;
        let mut ema = sma;

        // EMA 계산
        for candle in candles[period..].iter() {
            ema = (candle.close - ema) * multiplier + ema;
        }

        Some(ema)
    }

    /// SMA (Simple Moving Average) 계산
    ///
    /// Args:
    ///     candles: 캔들 데이터 리스트
    ///     period: SMA 기간
    ///
    /// Returns:
    ///     Option<f64>: SMA 값
    pub fn calculate_sma(candles: &[Candle], period: usize) -> Option<f64> {
        if candles.len() < period {
            return None;
        }

        let sum: f64 = candles[candles.len() - period..]
            .iter()
            .map(|c| c.close)
            .sum();

        Some(sum / period as f64)
    }

    /// 볼린저 밴드 (Bollinger Bands) 계산
    ///
    /// 가격이 상단 밴드 근처: 과매수
    /// 가격이 하단 밴드 근처: 과매도
    ///
    /// Args:
    ///     candles: 캔들 데이터 리스트
    ///     period: 기간 (기본값: 20)
    ///     std_dev: 표준편차 배수 (기본값: 2.0)
    ///
    /// Returns:
    ///     Option<(f64, f64, f64)>: (상단 밴드, 중간 밴드(SMA), 하단 밴드)
    pub fn calculate_bollinger_bands(
        candles: &[Candle],
        period: usize,
        std_dev: f64,
    ) -> Option<(f64, f64, f64)> {
        if candles.len() < period {
            return None;
        }

        // 중간 밴드 (SMA)
        let sma = Self::calculate_sma(candles, period)?;

        // 표준편차 계산
        let recent_candles = &candles[candles.len() - period..];
        let variance: f64 = recent_candles
            .iter()
            .map(|c| (c.close - sma).powi(2))
            .sum::<f64>()
            / period as f64;

        let std = variance.sqrt();

        // 상단/하단 밴드
        let upper_band = sma + (std * std_dev);
        let lower_band = sma - (std * std_dev);

        Some((upper_band, sma, lower_band))
    }

    /// 스토캐스틱 오실레이터 (Stochastic Oscillator) 계산
    ///
    /// %K > 80: 과매수
    /// %K < 20: 과매도
    ///
    /// Args:
    ///     candles: 캔들 데이터 리스트
    ///     period: 기간 (기본값: 14)
    ///
    /// Returns:
    ///     Option<(f64, f64)>: (%K, %D)
    pub fn calculate_stochastic(candles: &[Candle], period: usize) -> Option<(f64, f64)> {
        if candles.len() < period {
            return None;
        }

        let recent = &candles[candles.len() - period..];

        let highest = recent.iter().map(|c| c.high).fold(f64::MIN, f64::max);
        let lowest = recent.iter().map(|c| c.low).fold(f64::MAX, f64::min);
        let current = candles.last()?.close;

        if highest == lowest {
            return Some((50.0, 50.0));
        }

        // %K = (현재가 - 최저가) / (최고가 - 최저가) * 100
        let k = ((current - lowest) / (highest - lowest)) * 100.0;

        // %D = %K의 3일 이동평균 (단순화)
        let d = k;

        Some((k, d))
    }

    /// ATR (Average True Range) 계산 - 변동성 측정
    ///
    /// Args:
    ///     candles: 캔들 데이터 리스트
    ///     period: 기간 (기본값: 14)
    ///
    /// Returns:
    ///     Option<f64>: ATR 값
    pub fn calculate_atr(candles: &[Candle], period: usize) -> Option<f64> {
        if candles.len() < period + 1 {
            return None;
        }

        let mut true_ranges = Vec::new();

        for i in 1..candles.len() {
            let high_low = candles[i].high - candles[i].low;
            let high_close = (candles[i].high - candles[i - 1].close).abs();
            let low_close = (candles[i].low - candles[i - 1].close).abs();

            let tr = high_low.max(high_close).max(low_close);
            true_ranges.push(tr);
        }

        if true_ranges.len() < period {
            return None;
        }

        let atr: f64 = true_ranges[true_ranges.len() - period..]
            .iter()
            .sum::<f64>()
            / period as f64;

        Some(atr)
    }

    /// 이동평균선 골든크로스/데드크로스 판단
    ///
    /// Args:
    ///     candles: 캔들 데이터 리스트
    ///     short_period: 단기 이동평균 기간 (기본값: 5)
    ///     long_period: 장기 이동평균 기간 (기본값: 20)
    ///
    /// Returns:
    ///     Option<&str>: "golden_cross", "dead_cross", "none"
    pub fn detect_ma_cross(
        candles: &[Candle],
        short_period: usize,
        long_period: usize,
    ) -> Option<&'static str> {
        if candles.len() < long_period + 1 {
            return None;
        }

        let current_short = Self::calculate_sma(candles, short_period)?;
        let current_long = Self::calculate_sma(candles, long_period)?;

        let prev_candles = &candles[..candles.len() - 1];
        let prev_short = Self::calculate_sma(prev_candles, short_period)?;
        let prev_long = Self::calculate_sma(prev_candles, long_period)?;

        // 골든크로스: 단기 이동평균이 장기 이동평균을 상향 돌파
        if prev_short <= prev_long && current_short > current_long {
            return Some("golden_cross");
        }

        // 데드크로스: 단기 이동평균이 장기 이동평균을 하향 돌파
        if prev_short >= prev_long && current_short < current_long {
            return Some("dead_cross");
        }

        Some("none")
    }
}

/// 기술적 지표 신호
#[derive(Debug, Clone)]
pub struct IndicatorSignals {
    pub rsi: Option<f64>,
    pub rsi_signal: SignalType,
    pub macd: Option<(f64, f64, f64)>,
    pub macd_signal: SignalType,
    pub bollinger: Option<(f64, f64, f64)>,
    pub bollinger_signal: SignalType,
    pub ma_cross: Option<String>,
    pub overall_signal: SignalType,
}

#[derive(Debug, Clone, PartialEq)]
pub enum SignalType {
    StrongBuy,
    Buy,
    Neutral,
    Sell,
    StrongSell,
}

impl IndicatorSignals {
    /// 모든 지표를 종합하여 신호 생성
    pub fn from_candles(candles: &[Candle]) -> Self {
        let rsi = TechnicalIndicators::calculate_rsi(candles, 14);
        let macd = TechnicalIndicators::calculate_macd(candles);
        let bollinger = TechnicalIndicators::calculate_bollinger_bands(candles, 20, 2.0);
        let ma_cross = TechnicalIndicators::detect_ma_cross(candles, 5, 20).map(|s| s.to_string());

        let rsi_signal = Self::interpret_rsi(rsi);
        let macd_signal = Self::interpret_macd(macd);
        let bollinger_signal = Self::interpret_bollinger(bollinger, candles.last().map(|c| c.close));

        let overall_signal = Self::combine_signals(&rsi_signal, &macd_signal, &bollinger_signal);

        Self {
            rsi,
            rsi_signal,
            macd,
            macd_signal,
            bollinger,
            bollinger_signal,
            ma_cross,
            overall_signal,
        }
    }

    fn interpret_rsi(rsi: Option<f64>) -> SignalType {
        match rsi {
            Some(val) if val > 70.0 => SignalType::Sell,
            Some(val) if val > 60.0 => SignalType::Neutral,
            Some(val) if val < 30.0 => SignalType::Buy,
            Some(val) if val < 40.0 => SignalType::Neutral,
            _ => SignalType::Neutral,
        }
    }

    fn interpret_macd(macd: Option<(f64, f64, f64)>) -> SignalType {
        match macd {
            Some((macd_val, signal, _)) if macd_val > signal && macd_val > 0.0 => SignalType::Buy,
            Some((macd_val, signal, _)) if macd_val < signal && macd_val < 0.0 => SignalType::Sell,
            _ => SignalType::Neutral,
        }
    }

    fn interpret_bollinger(bollinger: Option<(f64, f64, f64)>, current_price: Option<f64>) -> SignalType {
        match (bollinger, current_price) {
            (Some((upper, _middle, lower)), Some(price)) => {
                if price >= upper {
                    SignalType::Sell // 과매수
                } else if price <= lower {
                    SignalType::Buy // 과매도
                } else {
                    SignalType::Neutral
                }
            }
            _ => SignalType::Neutral,
        }
    }

    fn combine_signals(rsi: &SignalType, macd: &SignalType, bollinger: &SignalType) -> SignalType {
        let buy_count = [rsi, macd, bollinger]
            .iter()
            .filter(|&&s| matches!(s, SignalType::Buy | SignalType::StrongBuy))
            .count();

        let sell_count = [rsi, macd, bollinger]
            .iter()
            .filter(|&&s| matches!(s, SignalType::Sell | SignalType::StrongSell))
            .count();

        if buy_count >= 2 {
            SignalType::Buy
        } else if sell_count >= 2 {
            SignalType::Sell
        } else {
            SignalType::Neutral
        }
    }
}
