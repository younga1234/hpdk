pub mod analyzer;
pub mod backtest;
pub mod config;
pub mod indicators;
pub mod trading;
pub mod upbit_client;
pub mod websocket;

use analyzer::CandleAnalyzer;
use config::Config;
use trading::TradingStrategy;
use upbit_client::UpbitClient;
use websocket::{CandleAggregator, TickerData, UpbitWebSocket};

use anyhow::Result;
use chrono::{DateTime, Utc};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};

#[tokio::main]
async fn main() -> Result<()> {
    // 로거 초기화
    env_logger::Builder::from_default_env()
        .filter_level(log::LevelFilter::Info)
        .init();

    log::info!("=== Upbit 자동매매봇 시작 ===");

    // 설정 로드
    let config = Arc::new(Config::from_env()?);
    log::info!("{}", config.display());

    // Upbit 클라이언트 초기화
    let client = Arc::new(UpbitClient::new(
        config.upbit_access_key.clone(),
        config.upbit_secret_key.clone(),
    ));

    // 초기 잔액 확인
    let initial_balance = client.get_balance("KRW").await?;
    log::info!("초기 잔액: {:.0}원", initial_balance);

    if initial_balance < config.min_krw_balance {
        anyhow::bail!(
            "잔액이 부족합니다 (현재: {:.0}원, 최소: {:.0}원)",
            initial_balance,
            config.min_krw_balance
        );
    }

    // KRW 마켓 조회
    log::info!("KRW 마켓 코드 조회 중...");
    let krw_markets = client.get_krw_markets().await?;
    log::info!("총 {} 개 KRW 마켓 발견", krw_markets.len());

    // 분석기 초기화
    let analyzer = CandleAnalyzer::new(
        config.min_volume_increase,
        config.min_price_change,
    );

    // 거래 전략 초기화
    let strategy = Arc::new(RwLock::new(TradingStrategy::new(
        client.clone(),
        analyzer,
        config.clone(),
    )));

    // 캔들 집계기 초기화
    let aggregator = Arc::new(CandleAggregator::new(config.candle_interval as i64));

    // WebSocket 메시지 채널
    let (tx, mut rx) = mpsc::unbounded_channel::<TickerData>();

    // WebSocket 클라이언트
    let ws = UpbitWebSocket::new();

    // WebSocket 연결 (백그라운드 태스크)
    let markets_clone = krw_markets.clone();
    tokio::spawn(async move {
        if let Err(e) = ws.connect(markets_clone, tx).await {
            log::error!("WebSocket 오류: {}", e);
        }
    });

    log::info!("실시간 데이터 수신 시작...");

    // 포지션 체크 주기 (10초)
    let strategy_clone = strategy.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(10));
        loop {
            interval.tick().await;
            let mut strat = strategy_clone.write().await;
            if strat.has_position() {
                if let Err(e) = strat.check_position().await {
                    log::error!("포지션 체크 오류: {}", e);
                }
            }
        }
    });

    // 잔액 출력 주기 (60초)
    let strategy_clone = strategy.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(60));
        loop {
            interval.tick().await;
            let strat = strategy_clone.read().await;
            if let Err(e) = strat.print_balance().await {
                log::error!("잔액 조회 오류: {}", e);
            }
        }
    });

    // 메인 루프: 틱 데이터 처리
    let mut last_candle_check: HashMap<String, DateTime<Utc>> = HashMap::new();

    while let Some(ticker_data) = rx.recv().await {
        let ticker = ticker_data.code.clone();
        let price = ticker_data.trade_price;
        let volume = ticker_data.trade_volume;
        let timestamp = DateTime::from_timestamp_millis(ticker_data.timestamp as i64)
            .unwrap_or_else(|| Utc::now());

        // 틱 데이터를 캔들로 집계
        if let Some(completed_candle) = aggregator
            .process_tick(ticker.clone(), price, volume, timestamp)
            .await
        {
            log::debug!(
                "[캔들완성] {} | O: {:.0} H: {:.0} L: {:.0} C: {:.0} | V: {:.0}",
                completed_candle.ticker,
                completed_candle.open,
                completed_candle.high,
                completed_candle.low,
                completed_candle.close,
                completed_candle.acc_trade_price
            );

            // 최근 완성된 캔들 조회 (기술적 지표를 위해 최소 20개 필요)
            let candles = aggregator.get_recent_candles(&ticker, 25).await;

            if candles.len() >= 20 {
                // 전략 실행
                let mut strat = strategy.write().await;

                // 포지션이 없을 때만 매수 신호 확인
                if !strat.has_position() {
                    // 너무 자주 체크하지 않도록 제한 (최소 1분 간격)
                    let should_check = if let Some(last_check) = last_candle_check.get(&ticker) {
                        (timestamp - *last_check).num_seconds() > 60
                    } else {
                        true
                    };

                    if should_check {
                        last_candle_check.insert(ticker.clone(), timestamp);

                        if let Err(e) = strat.execute_buy(&ticker, &candles).await {
                            log::error!("{}: 매수 실행 오류 - {}", ticker, e);
                        }
                    }
                }
            }
        }
    }

    log::info!("봇 종료");
    Ok(())
}
