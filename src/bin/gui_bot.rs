use upbit_trading_bot::{
    analyzer::CandleAnalyzer,
    config::Config,
    gui::{run_dashboard, DashboardState, TradeInfo},
    trading::TradingStrategy,
    upbit_client::UpbitClient,
    websocket::{CandleAggregator, TickerData, UpbitWebSocket},
};

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

    log::info!("=== Upbit 자동매매봇 (GUI 버전) 시작 ===");

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

    // 대시보드 상태 초기화
    let dashboard_state = Arc::new(RwLock::new(DashboardState {
        balance: initial_balance,
        total_asset: initial_balance,
        position_ticker: None,
        position_amount: 0.0,
        position_avg_price: 0.0,
        position_current_price: 0.0,
        position_profit_rate: 0.0,
        recent_trades: Vec::new(),
        monitoring_markets: krw_markets.clone(),
        bot_running: true,
    }));

    // 분석기 초기화
    let analyzer = CandleAnalyzer::new(config.min_volume_increase, config.min_price_change);

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

    // 트레이딩 봇 로직 (백그라운드)
    let strategy_bg = strategy.clone();
    let aggregator_bg = aggregator.clone();
    let dashboard_bg = dashboard_state.clone();
    let client_bg = client.clone();

    tokio::spawn(async move {
        let mut last_candle_check: HashMap<String, DateTime<Utc>> = HashMap::new();

        while let Some(ticker_data) = rx.recv().await {
            let ticker = ticker_data.code.clone();
            let price = ticker_data.trade_price;
            let volume = ticker_data.trade_volume;
            let timestamp = DateTime::from_timestamp_millis(ticker_data.timestamp as i64)
                .unwrap_or_else(|| Utc::now());

            // 틱 데이터를 캔들로 집계
            if let Some(_completed_candle) = aggregator_bg
                .process_tick(ticker.clone(), price, volume, timestamp)
                .await
            {
                let candles = aggregator_bg.get_recent_candles(&ticker, 25).await;

                if candles.len() >= 20 {
                    let mut strat = strategy_bg.write().await;

                    if !strat.has_position() {
                        let should_check = if let Some(last_check) = last_candle_check.get(&ticker)
                        {
                            (timestamp - *last_check).num_seconds() > 60
                        } else {
                            true
                        };

                        if should_check {
                            last_candle_check.insert(ticker.clone(), timestamp);

                            if let Ok(bought) = strat.execute_buy(&ticker, &candles).await {
                                if bought {
                                    // 대시보드 업데이트
                                    if let Some(pos) = strat.get_position_ticker() {
                                        let mut dash = dashboard_bg.write().await;
                                        dash.position_ticker = Some(pos.clone());
                                        dash.recent_trades.push(TradeInfo {
                                            ticker: pos,
                                            action: "BUY".to_string(),
                                            price,
                                            amount: 0.0,
                                            profit: None,
                                            time: chrono::Local::now().format("%H:%M:%S").to_string(),
                                        });
                                    }
                                }
                            }
                        }
                    }
                }
            }

            // 잔액 업데이트 (10초마다)
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        }
    });

    // 포지션 체크 주기
    let strategy_check = strategy.clone();
    let dashboard_check = dashboard_state.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(10));
        loop {
            interval.tick().await;
            let mut strat = strategy_check.write().await;
            if strat.has_position() {
                if let Err(e) = strat.check_position().await {
                    log::error!("포지션 체크 오류: {}", e);
                } else {
                    // 대시보드 업데이트
                    if let Some(ticker) = strat.get_position_ticker() {
                        let mut dash = dashboard_check.write().await;
                        dash.position_ticker = Some(ticker);
                    }
                }
            }
        }
    });

    // 잔액 업데이트 주기
    let dashboard_balance = dashboard_state.clone();
    let client_balance = client.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(5));
        loop {
            interval.tick().await;
            if let Ok(balance) = client_balance.get_balance("KRW").await {
                let mut dash = dashboard_balance.write().await;
                dash.balance = balance;
                dash.total_asset = balance; // 간단히 처리
            }
        }
    });

    // GUI 실행 (메인 스레드)
    log::info!("GUI 대시보드 실행...");
    run_dashboard(dashboard_state)?;

    Ok(())
}
