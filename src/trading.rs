use crate::analyzer::{CandleAnalyzer, PositionStatus};
use crate::config::Config;
use crate::upbit_client::UpbitClient;
use crate::websocket::Candle;
use anyhow::Result;
use chrono::{DateTime, Utc};
use std::sync::Arc;

/// 포지션 정보
#[derive(Debug, Clone)]
pub struct Position {
    pub ticker: String,
    pub amount: f64,
    pub avg_price: f64,
    pub timestamp: DateTime<Utc>,
    pub highest_price: f64,  // 트레일링 스톱용 최고가
    pub trailing_stop_enabled: bool,  // 트레일링 스톱 활성화 여부
}

impl Position {
    pub fn new(ticker: String, amount: f64, avg_price: f64) -> Self {
        Self {
            ticker,
            amount,
            avg_price,
            timestamp: Utc::now(),
            highest_price: avg_price,  // 초기값은 매수가
            trailing_stop_enabled: false,  // 초기에는 비활성화
        }
    }

    /// 최고가 업데이트 및 트레일링 스톱 체크
    pub fn update_trailing_stop(&mut self, current_price: f64, trailing_percent: f64) -> bool {
        // 최고가 갱신
        if current_price > self.highest_price {
            self.highest_price = current_price;
        }

        // 트레일링 스톱이 활성화되어 있으면 체크
        if self.trailing_stop_enabled {
            let drop_from_high = ((self.highest_price - current_price) / self.highest_price) * 100.0;
            return drop_from_high >= trailing_percent;
        }

        false
    }

    /// 목표 수익률 도달 시 트레일링 스톱 활성화
    pub fn enable_trailing_if_profit(&mut self, current_price: f64, trigger_profit: f64) {
        let profit_rate = self.get_profit_rate(current_price);
        if !self.trailing_stop_enabled && profit_rate >= trigger_profit {
            self.trailing_stop_enabled = true;
            self.highest_price = current_price;
            log::info!(
                "[트레일링스톱] {} 활성화 | 현재가: {:.0}원 | 수익률: {:.2}%",
                self.ticker,
                current_price,
                profit_rate
            );
        }
    }

    pub fn get_profit_rate(&self, current_price: f64) -> f64 {
        ((current_price - self.avg_price) / self.avg_price) * 100.0
    }

    pub fn get_profit_amount(&self, current_price: f64, fee_rate: f64) -> f64 {
        let (_, profit_amount) = CandleAnalyzer::calculate_profit(
            self.avg_price,
            current_price,
            self.amount,
            fee_rate,
        );
        profit_amount
    }
}

/// 거래 전략
pub struct TradingStrategy {
    client: Arc<UpbitClient>,
    analyzer: CandleAnalyzer,
    config: Arc<Config>,
    position: Option<Position>,
}

impl TradingStrategy {
    pub fn new(
        client: Arc<UpbitClient>,
        analyzer: CandleAnalyzer,
        config: Arc<Config>,
    ) -> Self {
        Self {
            client,
            analyzer,
            config,
            position: None,
        }
    }

    /// 매수 실행
    pub async fn execute_buy(&mut self, ticker: &str, candles: &[Candle]) -> Result<bool> {
        // 이미 포지션이 있으면 매수하지 않음
        if self.position.is_some() {
            return Ok(false);
        }

        // 매수 신호 확인
        let (should_buy, reason) = self.analyzer.analyze_buy_signal(candles);
        if !should_buy {
            return Ok(false);
        }

        // 잔액 확인
        let krw_balance = self.client.get_balance("KRW").await?;
        if krw_balance < self.config.min_krw_balance {
            log::warn!(
                "{}: 잔액 부족 (현재: {:.0}원)",
                ticker,
                krw_balance
            );
            return Ok(false);
        }

        // 매수 금액 계산
        let buy_amount = krw_balance * self.config.trade_amount;

        if buy_amount < 5000.0 {
            log::warn!(
                "{}: 최소 주문 금액 미달 (현재: {:.0}원)",
                ticker,
                buy_amount
            );
            return Ok(false);
        }

        // 현재가 조회
        let current_price = self.client.get_current_price(ticker).await?;

        // 매수 신호 로그
        log::info!(
            "[거래신호] {} | BUY | {}",
            ticker,
            reason.unwrap_or_default()
        );

        // 시장가 매수 주문
        match self.client.buy_market_order(ticker, buy_amount).await {
            Ok(order) => {
                log::info!("매수 주문 제출: {} | UUID: {}", ticker, order.uuid);

                // 주문 처리 대기
                tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;

                // 실제 매수된 수량과 평균가 확인
                let ticker_currency = ticker.replace("KRW-", "");
                let amount = self.client.get_balance(&ticker_currency).await?;

                if amount > 0.0 {
                    let avg_price = self
                        .client
                        .get_avg_buy_price(&ticker_currency)
                        .await?
                        .unwrap_or(current_price);

                    // 포지션 생성
                    self.position = Some(Position::new(ticker.to_string(), amount, avg_price));

                    log::info!(
                        "[거래실행] {} | BUY | 가격: {:.0}원 | 수량: {:.8} | 총액: {:.0}원",
                        ticker,
                        avg_price,
                        amount,
                        avg_price * amount
                    );

                    log::info!("✅ 매수 완료: {}", ticker);
                    return Ok(true);
                } else {
                    log::error!("{}: 매수 후 수량 확인 실패", ticker);
                    return Ok(false);
                }
            }
            Err(e) => {
                log::error!("{}: 매수 주문 실패 - {}", ticker, e);
                return Ok(false);
            }
        }
    }

    /// 매도 실행
    pub async fn execute_sell(&mut self, reason: &str) -> Result<bool> {
        let position = match &self.position {
            Some(p) => p.clone(),
            None => return Ok(false),
        };

        let ticker = &position.ticker;

        // 현재가 조회
        let current_price = self.client.get_current_price(ticker).await?;

        // 현재 보유 수량 확인
        let ticker_currency = ticker.replace("KRW-", "");
        let amount = self.client.get_balance(&ticker_currency).await?;

        if amount == 0.0 {
            log::warn!("{}: 보유 수량 없음", ticker);
            self.position = None;
            return Ok(false);
        }

        // 수익률 계산
        let profit_rate = position.get_profit_rate(current_price);
        let profit_amount = position.get_profit_amount(current_price, self.config.upbit_fee);

        log::info!(
            "[거래신호] {} | SELL | {} (수익률: {:.2}%, 수익금: {:+.0}원)",
            ticker,
            reason,
            profit_rate,
            profit_amount
        );

        // 시장가 매도 주문
        match self.client.sell_market_order(ticker, amount).await {
            Ok(_order) => {
                log::info!(
                    "[거래실행] {} | SELL | 가격: {:.0}원 | 수량: {:.8} | 총액: {:.0}원",
                    ticker,
                    current_price,
                    amount,
                    current_price * amount
                );

                log::info!(
                    "✅ 매도 완료: {} | 수익률: {:+.2}% | 수익금: {:+.0}원",
                    ticker,
                    profit_rate,
                    profit_amount
                );

                // 포지션 초기화
                self.position = None;
                return Ok(true);
            }
            Err(e) => {
                log::error!("{}: 매도 주문 실패 - {}", ticker, e);
                return Ok(false);
            }
        }
    }

    /// 포지션 상태 확인 및 매도 판단
    pub async fn check_position(&mut self) -> Result<bool> {
        let mut position = match &self.position {
            Some(p) => p.clone(),
            None => return Ok(false),
        };

        let ticker = &position.ticker;
        let current_price = self.client.get_current_price(ticker).await?;

        // 트레일링 스톱 (활성화 시)
        if self.config.trailing_stop_enabled {
            position.enable_trailing_if_profit(current_price, self.config.trailing_stop_trigger);
            if position.update_trailing_stop(current_price, self.config.trailing_stop_percent) {
                self.position = Some(position.clone());
                return self.execute_sell(&format!("트레일링 스톱 (최고가 {}원 대비 {}% 하락)", position.highest_price, self.config.trailing_stop_percent)).await;
            }
            self.position = Some(position.clone());
        }

        let (status, profit_rate, reason) = self.analyzer.analyze_position(current_price, position.avg_price, self.config.target_profit, self.config.stop_loss);
        let profit_amount = position.get_profit_amount(current_price, self.config.upbit_fee);
        log::info!("[포지션] {} | 평균가: {:.0}원 | 현재가: {:.0}원 | 수익: {:+.2}% ({:+.0}원)", ticker, position.avg_price, current_price, profit_rate, profit_amount);

        if status == PositionStatus::TakeProfit || status == PositionStatus::StopLoss {
            return self.execute_sell(&reason).await;
        }
        Ok(false)
    }

    /// 현재 포지션 보유 여부
    pub fn has_position(&self) -> bool {
        self.position.is_some()
    }

    /// 현재 포지션의 티커 반환
    pub fn get_position_ticker(&self) -> Option<String> {
        self.position.as_ref().map(|p| p.ticker.clone())
    }

    /// 잔액 정보 출력
    pub async fn print_balance(&self) -> Result<()> {
        let krw_balance = self.client.get_balance("KRW").await?;

        let mut total_asset = krw_balance;

        if let Some(position) = &self.position {
            let ticker = &position.ticker;
            let current_price = self.client.get_current_price(ticker).await?;
            let position_value = position.amount * current_price;
            total_asset += position_value;
        }

        log::info!(
            "[잔액] KRW: {:.0}원 | 총 자산: {:.0}원",
            krw_balance,
            total_asset
        );

        Ok(())
    }
}
