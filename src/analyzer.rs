use crate::websocket::Candle;
use crate::indicators::{IndicatorSignals, SignalType};

/// 포지션 상태
#[derive(Debug, Clone, PartialEq)]
pub enum PositionStatus {
    Hold,
    TakeProfit,
    StopLoss,
}

/// 캔들 분석기
pub struct CandleAnalyzer {
    min_volume_increase: f64,
    min_price_change: f64,
}

impl CandleAnalyzer {
    pub fn new(min_volume_increase: f64, min_price_change: f64) -> Self {
        Self {
            min_volume_increase,
            min_price_change,
        }
    }

    /// 매수 신호 분석 (개선된 버전 - 기술적 지표 통합)
    ///
    /// 수익성 개선을 위한 추가 조건:
    /// 1. 급등 초기 단계 포착 (완전히 상승하기 전)
    /// 2. 평균 거래량 대비 증가율 확인
    /// 3. 연속 상승 패턴 확인
    /// 4. 과매수 구간 필터링
    /// 5. RSI, MACD, Bollinger Bands 등 기술적 지표 활용
    pub fn analyze_buy_signal(&self, candles: &[Candle]) -> (bool, Option<String>) {
        if candles.len() < 20 {
            // 기술적 지표를 위해 최소 20개 캔들 필요
            return (false, None);
        }

        let prev_candle = &candles[candles.len() - 2];
        let current_candle = &candles[candles.len() - 1];

        // 1. 기본 조건: 거래량 증가
        let volume_ratio = self.calculate_volume_ratio(prev_candle, current_candle);
        if volume_ratio < self.min_volume_increase {
            return (false, None);
        }

        // 2. 가격 변화율 계산
        let prev_price_change = self.calculate_price_change_rate(prev_candle);
        let current_price_change = self.calculate_price_change_rate(current_candle);

        // 3. 현재 캔들이 양봉이고 상승 중
        if current_price_change <= self.min_price_change {
            return (false, None);
        }

        // 4. 가격 상승률이 이전보다 증가 (모멘텀 확인)
        if current_price_change <= prev_price_change {
            return (false, None);
        }

        // 5. 평균 거래량 대비 증가율 확인 (급등 초기 포착)
        let avg_volume = self.calculate_avg_volume(&candles[..candles.len() - 1]);
        let current_volume = current_candle.acc_trade_price;
        let volume_vs_avg = current_volume / avg_volume;

        // 평균 대비 1.5배 이상 증가 시만 신호
        if volume_vs_avg < 1.5 {
            return (false, None);
        }

        // 6. 과매수 필터링: 단기간에 너무 급등한 경우 제외
        let short_term_change = self.calculate_short_term_change(candles);
        if short_term_change > 10.0 {
            // 10% 이상 이미 상승한 경우 제외 (고점 매수 방지)
            return (false, None);
        }

        // 7. 연속 하락 후 반등 패턴 선호
        let is_reversal = self.is_reversal_pattern(candles);

        // 8. 기술적 지표 분석
        let indicator_signals = IndicatorSignals::from_candles(candles);

        // RSI 과매수/과매도 확인
        let rsi_ok = indicator_signals.rsi_signal != SignalType::Sell; // Sell = 과매수 구간 피함

        if !rsi_ok {
            return (false, Some("RSI 과매수 구간".to_string()));
        }

        // MACD 상승 신호 확인
        let macd_bullish = matches!(indicator_signals.macd_signal, SignalType::Buy | SignalType::StrongBuy);

        // 볼린저 밴드 하단 근처에서 반등 (매수 기회)
        let bb_buy_signal = matches!(indicator_signals.bollinger_signal, SignalType::Buy | SignalType::StrongBuy);

        // 이동평균선 골든크로스
        let ma_cross_bullish = indicator_signals.ma_cross.as_deref() == Some("골든크로스");

        // 지표 점수 계산 (0~4점)
        let mut indicator_score = 0;
        if macd_bullish {
            indicator_score += 1;
        }
        if bb_buy_signal {
            indicator_score += 1;
        }
        if ma_cross_bullish {
            indicator_score += 1;
        }
        if matches!(indicator_signals.rsi_signal, SignalType::Buy | SignalType::StrongBuy) {
            indicator_score += 1; // 과매도(Buy signal)는 추가 점수
        }

        // 최소 2개 이상의 긍정 신호 필요
        if indicator_score < 2 && !is_reversal {
            return (false, Some(format!("기술적 지표 신호 부족 (점수: {})", indicator_score)));
        }

        let reason = format!(
            "거래량: {:.2}배 (평균 대비 {:.2}배), 가격: {:.2}% → {:.2}%, 단기상승: {:.2}%, 반등: {}, 지표점수: {}/4 | RSI: {:?}, MACD: {:?}, BB: {:?}, MA: {}",
            volume_ratio,
            volume_vs_avg,
            prev_price_change,
            current_price_change,
            short_term_change,
            if is_reversal { "O" } else { "X" },
            indicator_score,
            indicator_signals.rsi_signal,
            indicator_signals.macd_signal,
            indicator_signals.bollinger_signal,
            indicator_signals.ma_cross.as_deref().unwrap_or("-"),
        );

        // 반등 패턴이거나 거래량이 매우 높거나 지표 점수가 3점 이상이면 매수
        let signal = is_reversal || volume_vs_avg > 2.0 || indicator_score >= 3;

        (signal, Some(reason))
    }

    /// 포지션 분석
    pub fn analyze_position(
        &self,
        current_price: f64,
        avg_buy_price: f64,
        target_profit: f64,
        stop_loss: f64,
    ) -> (PositionStatus, f64, String) {
        let profit_rate = ((current_price - avg_buy_price) / avg_buy_price) * 100.0;

        // 익절
        if profit_rate >= target_profit {
            return (
                PositionStatus::TakeProfit,
                profit_rate,
                format!("목표 수익률 달성 ({:.2}% >= {:.2}%)", profit_rate, target_profit),
            );
        }

        // 손절
        if profit_rate <= -stop_loss {
            return (
                PositionStatus::StopLoss,
                profit_rate,
                format!("손절 라인 도달 ({:.2}% <= -{:.2}%)", profit_rate, stop_loss),
            );
        }

        // 보유
        (
            PositionStatus::Hold,
            profit_rate,
            format!("현재 수익률: {:.2}%", profit_rate),
        )
    }

    /// 거래량 비율 계산
    fn calculate_volume_ratio(&self, prev: &Candle, current: &Candle) -> f64 {
        if prev.acc_trade_price == 0.0 {
            return 1.0;
        }
        current.acc_trade_price / prev.acc_trade_price
    }

    /// 가격 변화율 계산
    fn calculate_price_change_rate(&self, candle: &Candle) -> f64 {
        if candle.open == 0.0 {
            return 0.0;
        }
        ((candle.close - candle.open) / candle.open) * 100.0
    }

    /// 평균 거래량 계산
    fn calculate_avg_volume(&self, candles: &[Candle]) -> f64 {
        if candles.is_empty() {
            return 1.0;
        }

        let sum: f64 = candles.iter().map(|c| c.acc_trade_price).sum();
        sum / candles.len() as f64
    }

    /// 단기 가격 변화율 계산 (최근 3~5 캔들)
    fn calculate_short_term_change(&self, candles: &[Candle]) -> f64 {
        if candles.len() < 3 {
            return 0.0;
        }

        let start_idx = if candles.len() > 5 {
            candles.len() - 5
        } else {
            0
        };

        let start_price = candles[start_idx].open;
        let current_price = candles.last().unwrap().close;

        if start_price == 0.0 {
            return 0.0;
        }

        ((current_price - start_price) / start_price) * 100.0
    }

    /// 반등 패턴 확인 (연속 하락 후 상승)
    fn is_reversal_pattern(&self, candles: &[Candle]) -> bool {
        if candles.len() < 4 {
            return false;
        }

        // 최근 3개 캔들 확인
        let len = candles.len();
        let candle_1 = &candles[len - 3]; // 2개 전
        let candle_2 = &candles[len - 2]; // 1개 전
        let candle_3 = &candles[len - 1]; // 현재

        // 2개 전 캔들이 음봉
        let is_down_1 = candle_1.close < candle_1.open;
        // 1개 전 캔들이 음봉 또는 약한 양봉
        let is_down_2 = candle_2.close <= candle_2.open * 1.001;
        // 현재 캔들이 강한 양봉
        let is_up_3 = self.calculate_price_change_rate(candle_3) > self.min_price_change;

        is_down_1 && is_down_2 && is_up_3
    }

    /// 수익 계산 (수수료 포함)
    pub fn calculate_profit(
        avg_buy_price: f64,
        current_price: f64,
        amount: f64,
        fee_rate: f64,
    ) -> (f64, f64) {
        // 매수 금액
        let buy_total = avg_buy_price * amount;
        let buy_fee = buy_total * fee_rate;
        let total_buy_cost = buy_total + buy_fee;

        // 매도 금액
        let sell_total = current_price * amount;
        let sell_fee = sell_total * fee_rate;
        let total_sell_amount = sell_total - sell_fee;

        // 수익 계산
        let profit_amount = total_sell_amount - total_buy_cost;
        let profit_rate = (profit_amount / total_buy_cost) * 100.0;

        (profit_rate, profit_amount)
    }

    /// 시장 강도 분석
    pub fn get_market_strength(&self, candles: &[Candle]) -> MarketStrength {
        if candles.len() < 2 {
            return MarketStrength {
                volume_trend: 0.0,
                price_trend: 0.0,
                momentum: 0.0,
            };
        }

        // 거래량 추세
        let volumes: Vec<f64> = candles.iter().map(|c| c.acc_trade_price).collect();
        let volume_trend = if volumes[0] > 0.0 {
            (volumes.last().unwrap() - volumes[0]) / volumes[0] * 100.0
        } else {
            0.0
        };

        // 가격 추세
        let prices: Vec<f64> = candles.iter().map(|c| c.close).collect();
        let price_trend = if prices[0] > 0.0 {
            (prices.last().unwrap() - prices[0]) / prices[0] * 100.0
        } else {
            0.0
        };

        // 모멘텀
        let momentum = self.calculate_price_change_rate(candles.last().unwrap());

        MarketStrength {
            volume_trend,
            price_trend,
            momentum,
        }
    }
}

/// 시장 강도 구조체
#[derive(Debug, Clone)]
pub struct MarketStrength {
    pub volume_trend: f64,
    pub price_trend: f64,
    pub momentum: f64,
}
