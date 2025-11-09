use anyhow::Result;
use chrono::{DateTime, Timelike, Utc};
use futures::{SinkExt, StreamExt};
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use tokio_tungstenite::{connect_async, tungstenite::Message};

/// WebSocket 틱 데이터
#[derive(Debug, Deserialize, Clone)]
pub struct TickerData {
    #[serde(rename = "type")]
    pub type_field: String,
    pub code: String,
    pub trade_price: f64,
    pub trade_volume: f64,
    pub acc_trade_price_24h: f64,
    pub acc_trade_volume_24h: f64,
    pub timestamp: u64,
}

/// 캔들 데이터 구조체
#[derive(Debug, Clone)]
pub struct Candle {
    pub ticker: String,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: f64,
    pub acc_trade_price: f64,
    pub start_time: DateTime<Utc>,
    pub tick_count: u64,
}

/// 캔들 데이터 집계기
pub struct CandleAggregator {
    interval_minutes: i64,
    current_candles: Arc<RwLock<HashMap<String, CandleData>>>,
    completed_candles: Arc<RwLock<HashMap<String, Vec<Candle>>>>,
}

#[derive(Debug, Clone)]
struct CandleData {
    open: f64,
    high: f64,
    low: f64,
    close: f64,
    volume: f64,
    acc_trade_price: f64,
    start_time: DateTime<Utc>,
    tick_count: u64,
}

impl CandleAggregator {
    pub fn new(interval_minutes: i64) -> Self {
        Self {
            interval_minutes,
            current_candles: Arc::new(RwLock::new(HashMap::new())),
            completed_candles: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// 틱 데이터를 처리하고 캔들 생성
    pub async fn process_tick(
        &self,
        ticker: String,
        price: f64,
        volume: f64,
        timestamp: DateTime<Utc>,
    ) -> Option<Candle> {
        let interval_start = self.get_interval_start(timestamp);
        let mut current_candles = self.current_candles.write().await;

        let mut completed_candle = None;

        if let Some(candle_data) = current_candles.get_mut(&ticker) {
            // 새로운 캔들 간격이 시작되었는지 확인
            if candle_data.start_time != interval_start {
                // 이전 캔들 완성
                completed_candle = Some(Candle {
                    ticker: ticker.clone(),
                    open: candle_data.open,
                    high: candle_data.high,
                    low: candle_data.low,
                    close: candle_data.close,
                    volume: candle_data.volume,
                    acc_trade_price: candle_data.acc_trade_price,
                    start_time: candle_data.start_time,
                    tick_count: candle_data.tick_count,
                });

                // 완성된 캔들 저장
                let mut completed = self.completed_candles.write().await;
                let candles = completed.entry(ticker.clone()).or_insert_with(Vec::new);
                if let Some(ref candle) = completed_candle {
                    candles.push(candle.clone());
                    // 최근 10개만 유지
                    if candles.len() > 10 {
                        candles.remove(0);
                    }
                }

                // 새 캔들 시작
                *candle_data = CandleData {
                    open: price,
                    high: price,
                    low: price,
                    close: price,
                    volume,
                    acc_trade_price: price * volume,
                    start_time: interval_start,
                    tick_count: 1,
                };
            } else {
                // 기존 캔들 업데이트
                candle_data.high = candle_data.high.max(price);
                candle_data.low = candle_data.low.min(price);
                candle_data.close = price;
                candle_data.volume += volume;
                candle_data.acc_trade_price += price * volume;
                candle_data.tick_count += 1;
            }
        } else {
            // 새로운 티커의 첫 캔들 생성
            current_candles.insert(
                ticker.clone(),
                CandleData {
                    open: price,
                    high: price,
                    low: price,
                    close: price,
                    volume,
                    acc_trade_price: price * volume,
                    start_time: interval_start,
                    tick_count: 1,
                },
            );
        }

        completed_candle
    }

    /// 최근 완성된 캔들 조회
    pub async fn get_recent_candles(&self, ticker: &str, count: usize) -> Vec<Candle> {
        let completed = self.completed_candles.read().await;
        if let Some(candles) = completed.get(ticker) {
            let start = if candles.len() > count {
                candles.len() - count
            } else {
                0
            };
            candles[start..].to_vec()
        } else {
            Vec::new()
        }
    }

    /// 현재 진행 중인 캔들 조회
    pub async fn get_current_candle(&self, ticker: &str) -> Option<Candle> {
        let current = self.current_candles.read().await;
        current.get(ticker).map(|data| Candle {
            ticker: ticker.to_string(),
            open: data.open,
            high: data.high,
            low: data.low,
            close: data.close,
            volume: data.volume,
            acc_trade_price: data.acc_trade_price,
            start_time: data.start_time,
            tick_count: data.tick_count,
        })
    }

    fn get_interval_start(&self, timestamp: DateTime<Utc>) -> DateTime<Utc> {
        let minutes = (timestamp.minute() as i64 / self.interval_minutes) * self.interval_minutes;
        timestamp
            .with_minute(minutes as u32)
            .unwrap()
            .with_second(0)
            .unwrap()
            .with_nanosecond(0)
            .unwrap()
    }
}

/// Upbit WebSocket 클라이언트
pub struct UpbitWebSocket {
    url: String,
}

impl UpbitWebSocket {
    pub fn new() -> Self {
        Self {
            url: "wss://api.upbit.com/websocket/v1".to_string(),
        }
    }

    /// WebSocket 연결 및 데이터 수신
    pub async fn connect(
        &self,
        tickers: Vec<String>,
        tx: mpsc::UnboundedSender<TickerData>,
    ) -> Result<()> {
        loop {
            match self.connect_internal(&tickers, &tx).await {
                Ok(_) => {
                    log::info!("WebSocket 연결이 정상 종료되었습니다");
                    break;
                }
                Err(e) => {
                    log::error!("WebSocket 오류: {}. 5초 후 재연결합니다...", e);
                    tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
                }
            }
        }
        Ok(())
    }

    async fn connect_internal(
        &self,
        tickers: &[String],
        tx: &mpsc::UnboundedSender<TickerData>,
    ) -> Result<()> {
        let (ws_stream, _) = connect_async(&self.url).await?;
        log::info!("WebSocket 연결 성공");

        let (mut write, mut read) = ws_stream.split();

        // 구독 메시지 생성
        let subscribe_message = serde_json::json!([
            {"ticket": "upbit-trading-bot"},
            {
                "type": "ticker",
                "codes": tickers,
                "isOnlyRealtime": true
            },
            {"format": "DEFAULT"}
        ]);

        // 구독 메시지 전송
        write
            .send(Message::Text(subscribe_message.to_string()))
            .await?;

        log::info!("{} 개 코인 구독 시작", tickers.len());

        // 메시지 수신
        while let Some(message) = read.next().await {
            match message {
                Ok(Message::Binary(data)) => {
                    if let Ok(text) = String::from_utf8(data) {
                        if let Ok(ticker_data) = serde_json::from_str::<TickerData>(&text) {
                            if tx.send(ticker_data).is_err() {
                                log::error!("채널 전송 실패");
                                break;
                            }
                        }
                    }
                }
                Ok(Message::Close(_)) => {
                    log::info!("WebSocket 연결이 서버에 의해 종료되었습니다");
                    break;
                }
                Err(e) => {
                    log::error!("WebSocket 메시지 오류: {}", e);
                    break;
                }
                _ => {}
            }
        }

        Ok(())
    }
}
