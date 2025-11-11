use crate::analyzer::{CandleAnalyzer, PositionStatus};
use crate::config::Config;
use crate::market_conditions::{MarketCondition, MarketConditionAnalyzer, TradingStatistics};
use crate::upbit_client::UpbitClient;
use crate::websocket::Candle;
use anyhow::Result;
use chrono::{DateTime, Utc};
use std::sync::Arc;

/// 거래 포지션 정보
///
/// 매수한 자산의 상태를 추적하고 트레일링 스톱 로직을 관리합니다.
///
/// # Examples
///
/// ```
/// use upbit_trading_bot::trading::Position;
///
/// let mut position = Position::new("KRW-BTC".to_string(), 0.5, 50000.0);
/// position.enable_trailing_if_profit(51000.0, 2.0);
/// ```
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Position {
    pub ticker: String,
    pub amount: f64,
    pub avg_price: f64,
    pub timestamp: DateTime<Utc>,
    pub highest_price: f64,  // 트레일링 스톱용 최고가
    pub trailing_stop_enabled: bool,  // 트레일링 스톱 활성화 여부
}

impl Position {
    /// 새로운 포지션 생성
    ///
    /// # Arguments
    ///
    /// * `ticker` - 마켓 코드 (예: "KRW-BTC")
    /// * `amount` - 보유 수량
    /// * `avg_price` - 평균 매수가
    ///
    /// # Returns
    ///
    /// 초기화된 Position 인스턴스
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
    ///
    /// 현재 가격이 최고가를 경신하면 업데이트하고,
    /// 트레일링 스톱이 활성화된 경우 매도 조건을 확인합니다.
    ///
    /// # Arguments
    ///
    /// * `current_price` - 현재 시장 가격
    /// * `trailing_percent` - 최고가 대비 하락률 기준 (%)
    ///
    /// # Returns
    ///
    /// 트레일링 스톱 조건 충족 시 `true`, 아니면 `false`
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
    ///
    /// 수익률이 설정된 기준을 초과하면 트레일링 스톱을 자동으로 활성화합니다.
    ///
    /// # Arguments
    ///
    /// * `current_price` - 현재 시장 가격
    /// * `trigger_profit` - 트레일링 스톱 활성화 수익률 기준 (%)
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

/// 거래 전략 실행 엔진
///
/// Upbit API를 통해 실제 매수/매도를 실행하고
/// 포지션 관리 및 리스크 관리를 수행합니다.
///
/// # Examples
///
/// ```no_run
/// use std::sync::Arc;
/// use upbit_trading_bot::trading::TradingStrategy;
/// use upbit_trading_bot::analyzer::CandleAnalyzer;
/// use upbit_trading_bot::config::Config;
/// use upbit_trading_bot::upbit_client::UpbitClient;
///
/// # async fn example() -> anyhow::Result<()> {
/// let config = Arc::new(Config::from_env()?);
/// let client = Arc::new(UpbitClient::new("key".to_string(), "secret".to_string()));
/// let analyzer = CandleAnalyzer::new(1.5, 0.5);
/// let mut strategy = TradingStrategy::new(client, analyzer, config);
/// # Ok(())
/// # }
/// ```
pub struct TradingStrategy {
    client: Arc<UpbitClient>,
    analyzer: CandleAnalyzer,
    config: Arc<Config>,
    position: Option<Position>,
    market_analyzer: MarketConditionAnalyzer,
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
            market_analyzer: MarketConditionAnalyzer::new(),
        }
    }

    /// 매수 신호 확인 및 실행
    ///
    /// 캔들 데이터를 분석하여 매수 조건을 확인하고,
    /// 조건이 충족되면 시장가 매수 주문을 실행합니다.
    ///
    /// # Arguments
    ///
    /// * `ticker` - 매수할 마켓 코드 (예: "KRW-BTC")
    /// * `candles` - 분석에 사용할 캔들 데이터 (최소 20개 권장)
    ///
    /// # Returns
    ///
    /// 매수 성공 시 `Ok(true)`, 조건 미충족 또는 실패 시 `Ok(false)`, 에러 발생 시 `Err`
    ///
    /// # Errors
    ///
    /// API 호출 실패, 네트워크 오류, 인증 오류 등
    pub async fn execute_buy(&mut self, ticker: &str, candles: &[Candle]) -> Result<bool> {
        // 일일 통계 자동 리셋
        self.market_analyzer.auto_reset_daily_if_needed();

        // 이미 포지션이 있으면 매수하지 않음
        if self.position.is_some() {
            return Ok(false);
        }

        // ⭐ 실전 안전장치 0: Maximum Drawdown 체크
        let current_balance = self.calculate_total_balance().await?;
        self.market_analyzer.update_current_balance(current_balance);

        let drawdown_check = self.market_analyzer.check_drawdown_limit(
            current_balance,
            self.config.max_drawdown_percent
        );
        if !drawdown_check.can_trade {
            log::error!("[비상정지] {}", drawdown_check.reason);
            // 비상 정지: 포지션이 있으면 즉시 청산
            if self.position.is_some() {
                self.emergency_stop(&drawdown_check.reason).await?;
            }
            return Ok(false);
        }

        // ⭐ 실전 안전장치 1: 거래 가능 여부 확인
        let restriction = self.market_analyzer.can_trade();
        if !restriction.can_trade {
            log::warn!("[거래제한] {}", restriction.reason);
            return Ok(false);
        }

        // ⭐ 실전 안전장치 2: 시장 상황 분석
        let market_condition = self.market_analyzer.analyze_market_condition(candles);
        if market_condition == MarketCondition::Bearish {
            log::info!("[시장분석] 약세장 - 매수 보류");
            return Ok(false);
        }
        if market_condition == MarketCondition::HighVolatility {
            log::warn!("[시장분석] 고변동성 - 매수 위험");
            return Ok(false);
        }

        // ⭐ 실전 안전장치 3: 안전 점수 확인
        let safety_score = self.market_analyzer.calculate_safety_score();
        if safety_score < 50 {
            log::warn!("[안전점수] {}점 - 매수 보류 (최소 50점 필요)", safety_score);
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

                // 주문 체결 대기 및 확인 (최대 5회 재시도)
                let ticker_currency = ticker.replace("KRW-", "");
                let mut amount = 0.0;
                let mut avg_price = current_price;

                for attempt in 1..=5 {
                    tokio::time::sleep(tokio::time::Duration::from_millis(500 * attempt)).await;

                    // 주문 상태 확인
                    match self.client.get_order(&order.uuid).await {
                        Ok(order_status) => {
                            if order_status.state == "done" {
                                // 체결 완료
                                amount = self.client.get_balance(&ticker_currency).await?;
                                if let Some(price) = self.client.get_avg_buy_price(&ticker_currency).await? {
                                    avg_price = price;
                                }
                                break;
                            } else if order_status.state == "cancel" {
                                log::error!("{}: 주문이 취소되었습니다", ticker);
                                return Ok(false);
                            }
                            log::debug!("주문 상태 확인 중... (시도 {}/5, 상태: {})", attempt, order_status.state);
                        }
                        Err(e) => {
                            log::warn!("주문 상태 조회 실패 (시도 {}/5): {}", attempt, e);
                        }
                    }
                }

                if amount > 0.0 {
                    // 슬리피지 추적
                    let slippage = self.calculate_slippage(current_price, avg_price);
                    if slippage.abs() > 0.5 {
                        log::warn!("[슬리피지] {:.3}% (예상: {:.0}원, 체결: {:.0}원)",
                            slippage, current_price, avg_price);
                    }

                    // 포지션 생성
                    self.position = Some(Position::new(ticker.to_string(), amount, avg_price));

                    log::info!(
                        "[거래실행] {} | BUY | 가격: {:.0}원 | 수량: {:.8} | 총액: {:.0}원",
                        ticker,
                        avg_price,
                        amount,
                        avg_price * amount
                    );

                    // 포지션 상태 저장
                    if let Err(e) = self.save_position() {
                        log::warn!("포지션 저장 실패: {}", e);
                    }

                    log::info!("✅ 매수 완료: {}", ticker);
                    Ok(true)
                } else {
                    log::error!("{}: 매수 체결 확인 실패 (5초 초과)", ticker);
                    Ok(false)
                }
            }
            Err(e) => {
                log::error!("{}: 매수 주문 실패 - {}", ticker, e);
                Ok(false)
            }
        }
    }

    /// 포지션 청산 (매도)
    ///
    /// 현재 보유한 포지션을 시장가로 매도합니다.
    ///
    /// # Arguments
    ///
    /// * `reason` - 매도 사유 (로깅용)
    ///
    /// # Returns
    ///
    /// 매도 성공 시 `Ok(true)`, 포지션 없음 또는 실패 시 `Ok(false)`, 에러 발생 시 `Err`
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
            Ok(order) => {
                log::info!("매도 주문 제출: {} | UUID: {}", ticker, order.uuid);

                // 주문 체결 확인 (최대 5회 재시도)
                let mut sell_confirmed = false;
                for attempt in 1..=5 {
                    tokio::time::sleep(tokio::time::Duration::from_millis(500 * attempt)).await;

                    match self.client.get_order(&order.uuid).await {
                        Ok(order_status) => {
                            if order_status.state == "done" {
                                sell_confirmed = true;
                                break;
                            } else if order_status.state == "cancel" {
                                log::error!("{}: 매도 주문이 취소되었습니다", ticker);
                                return Ok(false);
                            }
                            log::debug!("매도 주문 상태 확인 중... (시도 {}/5, 상태: {})", attempt, order_status.state);
                        }
                        Err(e) => {
                            log::warn!("매도 주문 상태 조회 실패 (시도 {}/5): {}", attempt, e);
                        }
                    }
                }

                if sell_confirmed {
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

                    // ⭐ 거래 결과 기록
                    self.market_analyzer.record_trade(profit_rate);

                    // 통계 출력
                    let stats = self.market_analyzer.get_statistics();
                    log::info!(
                        "[거래통계] 승률: {:.1}%, 평균수익: {:+.2}%, 연속손실: {}, 일일거래: {}, 안전점수: {}",
                        stats.win_rate,
                        stats.avg_profit,
                        stats.consecutive_losses,
                        stats.daily_trade_count,
                        stats.safety_score
                    );

                    // 포지션 초기화
                    self.position = None;

                    // 포지션 상태 저장 (삭제)
                    if let Err(e) = self.save_position() {
                        log::warn!("포지션 상태 파일 삭제 실패: {}", e);
                    }

                    Ok(true)
                } else {
                    log::error!("{}: 매도 체결 확인 실패 (타임아웃)", ticker);
                    // 포지션은 유지 (재시도 가능)
                    Ok(false)
                }
            }
            Err(e) => {
                log::error!("{}: 매도 주문 실패 - {}", ticker, e);
                Ok(false)
            }
        }
    }

    /// 포지션 상태 확인 및 매도 판단
    ///
    /// 현재 포지션의 손익을 확인하고 익절/손절/트레일링 스톱 조건을 체크합니다.
    /// 매도 조건 충족 시 자동으로 매도를 실행합니다.
    ///
    /// # Returns
    ///
    /// 매도 실행 시 `Ok(true)`, 홀딩 시 `Ok(false)`, 에러 발생 시 `Err`
    pub async fn check_position(&mut self) -> Result<bool> {
        let mut position = match &self.position {
            Some(p) => p.clone(),
            None => return Ok(false),
        };

        let ticker = position.ticker.clone();
        let current_price = self.client.get_current_price(&ticker).await?;

        // 트레일링 스톱 (활성화 시)
        if self.config.trailing_stop_enabled {
            position.enable_trailing_if_profit(current_price, self.config.trailing_stop_trigger);
            let highest_price = position.highest_price;
            let trailing_percent = self.config.trailing_stop_percent;
            if position.update_trailing_stop(current_price, trailing_percent) {
                self.position = Some(position.clone());
                return self.execute_sell(&format!("트레일링 스톱 (최고가 {:.0}원 대비 {}% 하락)", highest_price, trailing_percent)).await;
            }
            self.position = Some(position.clone());
        }

        let (status, profit_rate, reason) = self.analyzer.analyze_position(current_price, position.avg_price, self.config.target_profit, self.config.stop_loss);
        let profit_amount = position.get_profit_amount(current_price, self.config.upbit_fee);
        let avg_price = position.avg_price;
        log::info!("[포지션] {} | 평균가: {:.0}원 | 현재가: {:.0}원 | 수익: {:+.2}% ({:+.0}원)", ticker, avg_price, current_price, profit_rate, profit_amount);

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

    /// 거래 통계 조회
    pub fn get_trading_statistics(&self) -> TradingStatistics {
        self.market_analyzer.get_statistics()
    }

    /// 안전 점수 조회
    pub fn get_safety_score(&self) -> u8 {
        self.market_analyzer.calculate_safety_score()
    }

    /// 포지션 상태를 파일에 저장
    pub fn save_position(&self) -> Result<()> {
        if let Some(position) = &self.position {
            let json = serde_json::to_string_pretty(position)?;
            std::fs::write("position_state.json", json)?;
            log::info!("포지션 상태 저장: {}", position.ticker);
        } else {
            // 포지션이 없으면 파일 삭제
            if std::path::Path::new("position_state.json").exists() {
                std::fs::remove_file("position_state.json")?;
                log::info!("포지션 상태 파일 삭제");
            }
        }
        Ok(())
    }

    /// 파일에서 포지션 상태 복구
    pub fn load_position(&mut self) -> Result<()> {
        if std::path::Path::new("position_state.json").exists() {
            let json = std::fs::read_to_string("position_state.json")?;
            let position: Position = serde_json::from_str(&json)?;
            self.position = Some(position.clone());
            log::info!("포지션 상태 복구: {} (평균가: {:.0}원, 수량: {:.8})",
                position.ticker, position.avg_price, position.amount);
        } else {
            log::info!("복구할 포지션 없음");
        }
        Ok(())
    }

    /// 슬리피지 계산 (예상가 vs 실제 체결가)
    pub fn calculate_slippage(&self, expected_price: f64, actual_price: f64) -> f64 {
        ((actual_price - expected_price) / expected_price) * 100.0
    }

    /// 비상 정지: 모든 포지션 즉시 청산
    pub async fn emergency_stop(&mut self, reason: &str) -> Result<bool> {
        if self.position.is_none() {
            return Ok(false);
        }

        log::error!("🚨 비상 정지 발동: {}", reason);
        log::error!("🚨 모든 포지션 즉시 청산 시작");

        let result = self.execute_sell(&format!("비상정지 - {}", reason)).await?;

        if result {
            log::error!("🚨 비상 청산 완료");
        } else {
            log::error!("🚨 비상 청산 실패 - 수동 확인 필요!");
        }

        Ok(result)
    }

    /// 총 자산 계산 (KRW + 포지션 평가액)
    pub async fn calculate_total_balance(&self) -> Result<f64> {
        let krw_balance = self.client.get_balance("KRW").await?;
        let mut total = krw_balance;

        if let Some(position) = &self.position {
            let current_price = self.client.get_current_price(&position.ticker).await?;
            let position_value = position.amount * current_price;
            total += position_value;
        }

        Ok(total)
    }

    /// 초기 자산 설정 (봇 시작 시 1회 호출)
    pub async fn initialize_balance(&mut self) -> Result<()> {
        let initial_balance = self.calculate_total_balance().await?;
        self.market_analyzer.set_initial_balance(initial_balance);
        Ok(())
    }

    /// 잔액 정보 출력
    pub async fn print_balance(&self) -> Result<()> {
        let krw_balance = self.client.get_balance("KRW").await?;
        let total_asset = self.calculate_total_balance().await?;

        // Drawdown 계산
        let drawdown_str = if let Some(drawdown) = self.market_analyzer.calculate_drawdown(total_asset) {
            format!("| Drawdown: {:.2}%", drawdown)
        } else {
            String::new()
        };

        log::info!(
            "[잔액] KRW: {:.0}원 | 총 자산: {:.0}원 {}",
            krw_balance,
            total_asset,
            drawdown_str
        );

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_position_creation() {
        let position = Position::new("KRW-BTC".to_string(), 0.5, 50000.0);
        assert_eq!(position.ticker, "KRW-BTC");
        assert_eq!(position.amount, 0.5);
        assert_eq!(position.avg_price, 50000.0);
        assert_eq!(position.highest_price, 50000.0);
        assert!(!position.trailing_stop_enabled);
    }

    #[test]
    fn test_profit_rate_calculation() {
        let position = Position::new("KRW-BTC".to_string(), 1.0, 50000.0);

        // 10% 이익
        let profit_rate = position.get_profit_rate(55000.0);
        assert!((profit_rate - 10.0).abs() < 0.01);

        // 5% 손실
        let loss_rate = position.get_profit_rate(47500.0);
        assert!((loss_rate - (-5.0)).abs() < 0.01);
    }

    #[test]
    fn test_trailing_stop_activation() {
        let mut position = Position::new("KRW-BTC".to_string(), 1.0, 50000.0);

        // 초기 상태: 트레일링 스톱 비활성화
        assert!(!position.trailing_stop_enabled);

        // 2% 수익 달성 시 트레일링 스톱 활성화
        position.enable_trailing_if_profit(51000.0, 2.0);
        assert!(position.trailing_stop_enabled);
        assert_eq!(position.highest_price, 51000.0);
    }

    #[test]
    fn test_trailing_stop_trigger() {
        let mut position = Position::new("KRW-BTC".to_string(), 1.0, 50000.0);

        // 트레일링 스톱 활성화
        position.trailing_stop_enabled = true;
        position.highest_price = 55000.0;

        // 최고가에서 2% 미만 하락 - 트리거 안됨
        let should_sell = position.update_trailing_stop(54000.0, 2.0);
        assert!(!should_sell);

        // 최고가에서 2% 이상 하락 - 트리거
        let should_sell = position.update_trailing_stop(53800.0, 2.0);
        assert!(should_sell);
    }

    #[test]
    fn test_highest_price_update() {
        let mut position = Position::new("KRW-BTC".to_string(), 1.0, 50000.0);
        position.trailing_stop_enabled = true;

        // 최고가 갱신
        position.update_trailing_stop(52000.0, 2.0);
        assert_eq!(position.highest_price, 52000.0);

        // 더 높은 가격으로 갱신
        position.update_trailing_stop(53000.0, 2.0);
        assert_eq!(position.highest_price, 53000.0);

        // 낮은 가격은 갱신되지 않음
        position.update_trailing_stop(52000.0, 2.0);
        assert_eq!(position.highest_price, 53000.0);
    }
}
