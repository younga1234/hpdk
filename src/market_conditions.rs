/// 업비트 시장 상황 분석 모듈
///
/// 실전 거래를 위한 시장 상황 판단 및 안전장치

use chrono::{DateTime, Timelike, Utc};
use std::collections::VecDeque;

/// 시장 상황
#[derive(Debug, Clone, PartialEq)]
pub enum MarketCondition {
    Bullish,      // 강세장
    Neutral,      // 중립
    Bearish,      // 약세장
    HighVolatility, // 고변동성 (위험)
}

/// 거래 제한 상태
#[derive(Debug, Clone)]
pub struct TradingRestriction {
    pub can_trade: bool,
    pub reason: String,
}

/// 시장 상황 분석기
pub struct MarketConditionAnalyzer {
    /// 최근 거래 결과 (수익률)
    recent_trades: VecDeque<f64>,
    max_trade_history: usize,
    /// 연속 손실 카운트
    consecutive_losses: usize,
    /// 일일 거래 횟수
    daily_trade_count: usize,
    /// 마지막 거래 시간
    last_trade_time: Option<DateTime<Utc>>,
    /// 초기 자산 (Maximum Drawdown 계산용)
    initial_balance: Option<f64>,
    /// 최대 자산 (Peak)
    peak_balance: Option<f64>,
    /// 마지막 일일 리셋 날짜
    last_reset_date: Option<String>,
}

impl MarketConditionAnalyzer {
    pub fn new() -> Self {
        Self {
            recent_trades: VecDeque::new(),
            max_trade_history: 10,
            consecutive_losses: 0,
            daily_trade_count: 0,
            last_trade_time: None,
            initial_balance: None,
            peak_balance: None,
            last_reset_date: None,
        }
    }

    /// 초기 자산 설정
    pub fn set_initial_balance(&mut self, balance: f64) {
        if self.initial_balance.is_none() {
            self.initial_balance = Some(balance);
            self.peak_balance = Some(balance);
            log::info!("[초기자산] {:.0}원 설정", balance);
        }
    }

    /// 현재 자산 업데이트 (Peak 추적)
    pub fn update_current_balance(&mut self, balance: f64) {
        if let Some(peak) = self.peak_balance {
            if balance > peak {
                self.peak_balance = Some(balance);
                log::debug!("[최고자산] {:.0}원 갱신", balance);
            }
        } else {
            self.peak_balance = Some(balance);
        }
    }

    /// Maximum Drawdown 계산 (%)
    pub fn calculate_drawdown(&self, current_balance: f64) -> Option<f64> {
        self.peak_balance.map(|peak| {
            if peak > 0.0 {
                ((peak - current_balance) / peak) * 100.0
            } else {
                0.0
            }
        })
    }

    /// Drawdown이 위험 수준인지 확인
    pub fn check_drawdown_limit(&self, current_balance: f64, max_drawdown_percent: f64) -> TradingRestriction {
        if let Some(drawdown) = self.calculate_drawdown(current_balance) {
            if drawdown >= max_drawdown_percent {
                return TradingRestriction {
                    can_trade: false,
                    reason: format!(
                        "최대 손실 한도 초과 (Drawdown: {:.2}%, 한도: {:.2}%)",
                        drawdown, max_drawdown_percent
                    ),
                };
            }
        }
        TradingRestriction {
            can_trade: true,
            reason: "정상".to_string(),
        }
    }

    /// 일일 통계 자동 리셋 (자정 체크)
    pub fn auto_reset_daily_if_needed(&mut self) {
        let today = Utc::now().format("%Y-%m-%d").to_string();

        if let Some(last_date) = &self.last_reset_date {
            if last_date != &today {
                self.reset_daily_counter();
                self.last_reset_date = Some(today);
            }
        } else {
            self.last_reset_date = Some(today);
        }
    }

    /// 거래 가능 여부 확인 (업비트 실전 최적화)
    pub fn can_trade(&self) -> TradingRestriction {
        // 1. 연속 손실 제한 (3회 이상 손실 시 1시간 대기)
        if self.consecutive_losses >= 3 {
            if let Some(last_time) = self.last_trade_time {
                let elapsed = Utc::now().signed_duration_since(last_time);
                if elapsed.num_minutes() < 60 {
                    return TradingRestriction {
                        can_trade: false,
                        reason: format!(
                            "연속 {}회 손실. {}분 후 재개 가능",
                            self.consecutive_losses,
                            60 - elapsed.num_minutes()
                        ),
                    };
                }
            }
        }

        // 2. 일일 거래 횟수 제한 (과도한 거래 방지)
        if self.daily_trade_count >= 20 {
            return TradingRestriction {
                can_trade: false,
                reason: format!("일일 거래 한도 초과 ({}/20)", self.daily_trade_count),
            };
        }

        // 3. 시간대 필터 (한국 시간 새벽 1시~5시는 거래량 부족)
        let hour = Utc::now().hour();
        if (16..20).contains(&hour) { // UTC 16~20시 = KST 01~05시
            return TradingRestriction {
                can_trade: false,
                reason: "새벽 시간대 (거래량 부족)".to_string(),
            };
        }

        // 4. 최근 거래 성적이 너무 나쁜 경우
        if self.recent_trades.len() >= 5 {
            let recent_avg: f64 = self.recent_trades.iter().sum::<f64>() / self.recent_trades.len() as f64;
            if recent_avg < -2.0 {
                return TradingRestriction {
                    can_trade: false,
                    reason: format!("최근 평균 수익률 부진 ({:.2}%)", recent_avg),
                };
            }
        }

        TradingRestriction {
            can_trade: true,
            reason: "정상".to_string(),
        }
    }

    /// 거래 결과 기록
    pub fn record_trade(&mut self, profit_rate: f64) {
        // 최근 거래 기록
        if self.recent_trades.len() >= self.max_trade_history {
            self.recent_trades.pop_front();
        }
        self.recent_trades.push_back(profit_rate);

        // 연속 손실 카운트
        if profit_rate < 0.0 {
            self.consecutive_losses += 1;
        } else {
            self.consecutive_losses = 0; // 수익 나면 리셋
        }

        // 일일 거래 횟수
        self.daily_trade_count += 1;
        self.last_trade_time = Some(Utc::now());
    }

    /// 일일 카운터 리셋 (자정마다 호출)
    pub fn reset_daily_counter(&mut self) {
        self.daily_trade_count = 0;
    }

    /// 변동성 체크 (업비트 특성)
    pub fn check_volatility(&self, price_changes: &[f64]) -> bool {
        if price_changes.is_empty() {
            return false;
        }

        // 최근 가격 변동폭 계산
        let max_change = price_changes.iter().map(|x| x.abs()).fold(0.0_f64, f64::max);

        // 5분 내 5% 이상 변동은 고변동성으로 판단
        if max_change > 5.0 {
            log::warn!("고변동성 감지: {:.2}% 변동", max_change);
            return true;
        }

        // 표준편차 기반 변동성
        let mean: f64 = price_changes.iter().sum::<f64>() / price_changes.len() as f64;
        let variance: f64 = price_changes.iter()
            .map(|x| (x - mean).powi(2))
            .sum::<f64>() / price_changes.len() as f64;
        let std_dev = variance.sqrt();

        // 표준편차가 2.0 이상이면 변동성 높음
        std_dev > 2.0
    }

    /// 시장 상황 판단
    pub fn analyze_market_condition(
        &self,
        recent_candles: &[crate::websocket::Candle],
    ) -> MarketCondition {
        if recent_candles.len() < 10 {
            return MarketCondition::Neutral;
        }

        // 최근 10개 캔들의 가격 변화
        let price_changes: Vec<f64> = recent_candles
            .iter()
            .map(|c| ((c.close - c.open) / c.open) * 100.0)
            .collect();

        // 고변동성 체크
        if self.check_volatility(&price_changes) {
            return MarketCondition::HighVolatility;
        }

        // 상승/하락 캔들 카운트
        let bullish_count = price_changes.iter().filter(|&&x| x > 0.0).count();
        let bearish_count = price_changes.len() - bullish_count;

        // 전체 가격 변화
        let first_price = recent_candles[0].open;
        let last_price = recent_candles.last().unwrap().close;
        let total_change = ((last_price - first_price) / first_price) * 100.0;

        // 강세장: 70% 이상 양봉 + 3% 이상 상승
        if bullish_count as f64 / price_changes.len() as f64 > 0.7 && total_change > 3.0 {
            return MarketCondition::Bullish;
        }

        // 약세장: 70% 이상 음봉 + 3% 이상 하락
        if bearish_count as f64 / price_changes.len() as f64 > 0.7 && total_change < -3.0 {
            return MarketCondition::Bearish;
        }

        MarketCondition::Neutral
    }

    /// 안전 점수 계산 (0~100, 높을수록 안전)
    pub fn calculate_safety_score(&self) -> u8 {
        let mut score: u8 = 100;

        // 연속 손실 차감
        score = score.saturating_sub(self.consecutive_losses as u8 * 15);

        // 최근 거래 성적
        if self.recent_trades.len() >= 3 {
            let recent_avg: f64 = self.recent_trades.iter().sum::<f64>() / self.recent_trades.len() as f64;
            if recent_avg < -1.0 {
                score = score.saturating_sub(20);
            } else if recent_avg < 0.0 {
                score = score.saturating_sub(10);
            }
        }

        // 일일 거래 횟수 (과도한 거래 차감)
        if self.daily_trade_count > 15 {
            score = score.saturating_sub(15);
        } else if self.daily_trade_count > 10 {
            score = score.saturating_sub(10);
        }

        score
    }

    /// 통계 정보
    pub fn get_statistics(&self) -> TradingStatistics {
        let total_trades = self.recent_trades.len();
        let winning_trades = self.recent_trades.iter().filter(|&&x| x > 0.0).count();
        let losing_trades = total_trades - winning_trades;

        let win_rate = if total_trades > 0 {
            (winning_trades as f64 / total_trades as f64) * 100.0
        } else {
            0.0
        };

        let avg_profit = if total_trades > 0 {
            self.recent_trades.iter().sum::<f64>() / total_trades as f64
        } else {
            0.0
        };

        TradingStatistics {
            total_trades,
            winning_trades,
            losing_trades,
            win_rate,
            avg_profit,
            consecutive_losses: self.consecutive_losses,
            daily_trade_count: self.daily_trade_count,
            safety_score: self.calculate_safety_score(),
            current_drawdown: None, // 외부에서 설정
            peak_balance: self.peak_balance,
        }
    }

    /// 통계 정보 (자산 포함)
    pub fn get_statistics_with_balance(&self, current_balance: f64) -> TradingStatistics {
        let mut stats = self.get_statistics();
        stats.current_drawdown = self.calculate_drawdown(current_balance);
        stats
    }
}

impl Default for MarketConditionAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

/// 거래 통계
#[derive(Debug, Clone)]
pub struct TradingStatistics {
    pub total_trades: usize,
    pub winning_trades: usize,
    pub losing_trades: usize,
    pub win_rate: f64,
    pub avg_profit: f64,
    pub consecutive_losses: usize,
    pub daily_trade_count: usize,
    pub safety_score: u8,
    pub current_drawdown: Option<f64>,
    pub peak_balance: Option<f64>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_consecutive_losses_restriction() {
        let mut analyzer = MarketConditionAnalyzer::new();

        // 3회 연속 손실 기록
        analyzer.record_trade(-1.0);
        analyzer.record_trade(-0.5);
        analyzer.record_trade(-1.5);

        let restriction = analyzer.can_trade();
        assert!(!restriction.can_trade);
        assert!(restriction.reason.contains("연속"));
    }

    #[test]
    fn test_daily_trade_limit() {
        let mut analyzer = MarketConditionAnalyzer::new();

        // 20회 거래 기록
        for _ in 0..20 {
            analyzer.record_trade(1.0);
        }

        let restriction = analyzer.can_trade();
        assert!(!restriction.can_trade);
        assert!(restriction.reason.contains("한도"));
    }

    #[test]
    fn test_volatility_check() {
        let analyzer = MarketConditionAnalyzer::new();

        // 정상 변동성
        let normal_changes = vec![0.5, -0.3, 0.8, -0.2, 0.4];
        assert!(!analyzer.check_volatility(&normal_changes));

        // 고변동성
        let high_changes = vec![6.0, -5.5, 7.0, -4.0, 8.0];
        assert!(analyzer.check_volatility(&high_changes));
    }

    #[test]
    fn test_safety_score() {
        let mut analyzer = MarketConditionAnalyzer::new();

        // 초기 점수
        assert_eq!(analyzer.calculate_safety_score(), 100);

        // 손실 후 점수 감소
        analyzer.record_trade(-2.0);
        analyzer.record_trade(-1.5);
        assert!(analyzer.calculate_safety_score() < 100);
    }
}
