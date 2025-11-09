use crate::analyzer::CandleAnalyzer;
use crate::config::Config;
use crate::websocket::Candle;
use anyhow::Result;
use chrono::{DateTime, Utc};

/// 백테스팅 거래 기록
#[derive(Debug, Clone)]
pub struct Trade {
    pub ticker: String,
    pub entry_time: DateTime<Utc>,
    pub exit_time: DateTime<Utc>,
    pub entry_price: f64,
    pub exit_price: f64,
    pub amount: f64,
    pub profit_rate: f64,
    pub profit_amount: f64,
    pub reason: String,
}

/// 백테스팅 통계
#[derive(Debug, Clone)]
pub struct BacktestStats {
    pub total_trades: usize,
    pub winning_trades: usize,
    pub losing_trades: usize,
    pub win_rate: f64,
    pub total_profit: f64,
    pub total_profit_rate: f64,
    pub max_profit: f64,
    pub max_loss: f64,
    pub average_profit: f64,
    pub average_holding_time: f64, // 시간 단위
}

/// 백테스팅 엔진
pub struct BacktestEngine {
    analyzer: CandleAnalyzer,
    config: Config,
    initial_balance: f64,
    current_balance: f64,
    trades: Vec<Trade>,
}

impl BacktestEngine {
    pub fn new(analyzer: CandleAnalyzer, config: Config, initial_balance: f64) -> Self {
        Self {
            analyzer,
            config,
            initial_balance,
            current_balance: initial_balance,
            trades: Vec::new(),
        }
    }

    /// 백테스팅 실행
    pub fn run(&mut self, candles: &[Candle]) -> Result<BacktestStats> {
        let mut position: Option<(usize, f64, f64)> = None; // (entry_idx, amount, avg_price)

        // 최소 20개 캔들 필요
        if candles.len() < 20 {
            anyhow::bail!("백테스팅을 위해 최소 20개 이상의 캔들이 필요합니다");
        }

        for i in 20..candles.len() {
            let window = &candles[i.saturating_sub(25)..=i];

            if let Some((entry_idx, amount, avg_price)) = position {
                // 포지션 보유 중 - 매도 신호 확인
                let current_candle = &candles[i];
                let current_price = current_candle.close;

                let profit_rate = ((current_price - avg_price) / avg_price) * 100.0;

                // 익절 또는 손절 확인
                let should_sell = profit_rate >= self.config.target_profit
                    || profit_rate <= -self.config.stop_loss;

                if should_sell {
                    // 매도 실행
                    let sell_total = current_price * amount;
                    let sell_fee = sell_total * self.config.upbit_fee;
                    let net_sell = sell_total - sell_fee;

                    let buy_total = avg_price * amount;
                    let buy_fee = buy_total * self.config.upbit_fee;
                    let total_cost = buy_total + buy_fee;

                    let profit_amount = net_sell - total_cost;
                    let actual_profit_rate = (profit_amount / total_cost) * 100.0;

                    self.current_balance += net_sell;

                    let reason = if profit_rate >= self.config.target_profit {
                        format!("익절 ({}%)", self.config.target_profit)
                    } else {
                        format!("손절 ({}%)", self.config.stop_loss)
                    };

                    self.trades.push(Trade {
                        ticker: current_candle.ticker.clone(),
                        entry_time: candles[entry_idx].start_time,
                        exit_time: current_candle.start_time,
                        entry_price: avg_price,
                        exit_price: current_price,
                        amount,
                        profit_rate: actual_profit_rate,
                        profit_amount,
                        reason,
                    });

                    position = None;
                }
            } else {
                // 포지션 없음 - 매수 신호 확인
                if window.len() < 20 {
                    continue;
                }

                let (should_buy, _reason) = self.analyzer.analyze_buy_signal(window);

                if should_buy && self.current_balance >= self.config.min_krw_balance {
                    // 매수 실행
                    let buy_amount = self.current_balance * self.config.trade_amount;

                    if buy_amount >= 5000.0 {
                        let current_candle = &candles[i];
                        let buy_price = current_candle.close;

                        let fee = buy_amount * self.config.upbit_fee;
                        let total_cost = buy_amount + fee;

                        if total_cost <= self.current_balance {
                            self.current_balance -= total_cost;
                            let amount = buy_amount / buy_price;

                            position = Some((i, amount, buy_price));
                        }
                    }
                }
            }
        }

        // 마지막에 포지션이 남아있으면 강제 청산
        if let Some((entry_idx, amount, avg_price)) = position {
            let last_candle = candles.last().unwrap();
            let current_price = last_candle.close;

            let sell_total = current_price * amount;
            let sell_fee = sell_total * self.config.upbit_fee;
            let net_sell = sell_total - sell_fee;

            let buy_total = avg_price * amount;
            let buy_fee = buy_total * self.config.upbit_fee;
            let total_cost = buy_total + buy_fee;

            let profit_amount = net_sell - total_cost;
            let actual_profit_rate = (profit_amount / total_cost) * 100.0;

            self.current_balance += net_sell;

            self.trades.push(Trade {
                ticker: last_candle.ticker.clone(),
                entry_time: candles[entry_idx].start_time,
                exit_time: last_candle.start_time,
                entry_price: avg_price,
                exit_price: current_price,
                amount,
                profit_rate: actual_profit_rate,
                profit_amount,
                reason: "백테스트 종료 - 강제 청산".to_string(),
            });
        }

        Ok(self.calculate_stats())
    }

    /// 통계 계산
    fn calculate_stats(&self) -> BacktestStats {
        if self.trades.is_empty() {
            return BacktestStats {
                total_trades: 0,
                winning_trades: 0,
                losing_trades: 0,
                win_rate: 0.0,
                total_profit: 0.0,
                total_profit_rate: 0.0,
                max_profit: 0.0,
                max_loss: 0.0,
                average_profit: 0.0,
                average_holding_time: 0.0,
            };
        }

        let total_trades = self.trades.len();
        let winning_trades = self.trades.iter().filter(|t| t.profit_amount > 0.0).count();
        let losing_trades = total_trades - winning_trades;
        let win_rate = (winning_trades as f64 / total_trades as f64) * 100.0;

        let total_profit: f64 = self.trades.iter().map(|t| t.profit_amount).sum();
        let total_profit_rate = ((self.current_balance - self.initial_balance) / self.initial_balance) * 100.0;

        let max_profit = self.trades.iter()
            .map(|t| t.profit_amount)
            .max_by(|a, b| a.partial_cmp(b).unwrap())
            .unwrap_or(0.0);

        let max_loss = self.trades.iter()
            .map(|t| t.profit_amount)
            .min_by(|a, b| a.partial_cmp(b).unwrap())
            .unwrap_or(0.0);

        let average_profit = total_profit / total_trades as f64;

        let total_holding_time: f64 = self.trades.iter()
            .map(|t| (t.exit_time - t.entry_time).num_hours() as f64)
            .sum();
        let average_holding_time = total_holding_time / total_trades as f64;

        BacktestStats {
            total_trades,
            winning_trades,
            losing_trades,
            win_rate,
            total_profit,
            total_profit_rate,
            max_profit,
            max_loss,
            average_profit,
            average_holding_time,
        }
    }

    /// 거래 기록 출력
    pub fn print_trades(&self) {
        println!("\n===== 거래 기록 =====");
        for (idx, trade) in self.trades.iter().enumerate() {
            println!(
                "#{} | {} | 진입: {:.0}원 → 청산: {:.0}원 | 수익: {:+.2}% ({:+.0}원) | {}",
                idx + 1,
                trade.ticker,
                trade.entry_price,
                trade.exit_price,
                trade.profit_rate,
                trade.profit_amount,
                trade.reason
            );
        }
    }

    /// 통계 출력
    pub fn print_stats(&self, stats: &BacktestStats) {
        println!("\n===== 백테스팅 결과 =====");
        println!("초기 자본: {:.0}원", self.initial_balance);
        println!("최종 자본: {:.0}원", self.current_balance);
        println!("총 수익: {:+.0}원 ({:+.2}%)", stats.total_profit, stats.total_profit_rate);
        println!();
        println!("총 거래 횟수: {}", stats.total_trades);
        println!("승리 거래: {} ({:.1}%)", stats.winning_trades, stats.win_rate);
        println!("패배 거래: {}", stats.losing_trades);
        println!();
        println!("최대 수익: {:+.0}원", stats.max_profit);
        println!("최대 손실: {:+.0}원", stats.max_loss);
        println!("평균 수익: {:+.0}원", stats.average_profit);
        println!("평균 보유 시간: {:.1}시간", stats.average_holding_time);
        println!("=========================");
    }

    pub fn get_trades(&self) -> &[Trade] {
        &self.trades
    }

    pub fn get_current_balance(&self) -> f64 {
        self.current_balance
    }
}
