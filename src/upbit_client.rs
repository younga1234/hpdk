use anyhow::Result;
use base64::{Engine as _, engine::general_purpose};
use hmac::{Hmac, Mac};
use reqwest::{Client, header};
use serde::{Deserialize, Serialize};
use sha2::Sha512;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH, Duration};
use tokio::sync::Mutex;
use uuid::Uuid;

type HmacSha512 = Hmac<Sha512>;

/// Upbit API 에러 응답
#[derive(Debug, Deserialize)]
struct UpbitErrorResponse {
    error: UpbitError,
}

#[derive(Debug, Deserialize)]
struct UpbitError {
    message: String,
    name: String,
}

/// Rate Limiter (간단한 구현)
struct RateLimiter {
    last_request: Arc<Mutex<SystemTime>>,
    min_interval_ms: u64,
}

impl RateLimiter {
    fn new(requests_per_second: u64) -> Self {
        Self {
            last_request: Arc::new(Mutex::new(SystemTime::now())),
            min_interval_ms: 1000 / requests_per_second,
        }
    }

    async fn wait(&self) {
        let mut last = self.last_request.lock().await;
        let now = SystemTime::now();

        if let Ok(elapsed) = now.duration_since(*last) {
            let elapsed_ms = elapsed.as_millis() as u64;
            if elapsed_ms < self.min_interval_ms {
                let sleep_ms = self.min_interval_ms - elapsed_ms;
                tokio::time::sleep(Duration::from_millis(sleep_ms)).await;
            }
        }

        *last = SystemTime::now();
    }
}

/// Upbit API 클라이언트
#[derive(Clone)]
pub struct UpbitClient {
    access_key: String,
    secret_key: String,
    client: Client,
    base_url: String,
    rate_limiter: Arc<RateLimiter>,
}

/// 잔액 정보
#[derive(Debug, Deserialize, Clone)]
pub struct Balance {
    pub currency: String,
    pub balance: String,
    pub locked: String,
    pub avg_buy_price: String,
    pub avg_buy_price_modified: bool,
    pub unit_currency: String,
}

/// 주문 정보
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Order {
    pub uuid: String,
    pub side: String,
    pub ord_type: String,
    pub price: Option<String>,
    pub state: String,
    pub market: String,
    pub created_at: String,
    pub volume: Option<String>,
    pub remaining_volume: Option<String>,
    pub reserved_fee: Option<String>,
    pub remaining_fee: Option<String>,
    pub paid_fee: Option<String>,
    pub locked: Option<String>,
    pub executed_volume: Option<String>,
    pub trades_count: Option<i32>,
}

/// 현재가 정보
#[derive(Debug, Deserialize, Clone)]
pub struct Ticker {
    pub market: String,
    pub trade_price: f64,
    pub trade_volume: f64,
    pub acc_trade_price_24h: f64,
    pub acc_trade_volume_24h: f64,
    pub prev_closing_price: f64,
    pub change_rate: f64,
}

/// 마켓 코드 정보
#[derive(Debug, Deserialize, Clone)]
pub struct MarketCode {
    pub market: String,
    pub korean_name: String,
    pub english_name: String,
}

/// 캔들 데이터 (API 응답)
#[derive(Debug, Deserialize, Clone)]
pub struct Candle {
    pub market: String,
    pub candle_date_time_utc: String,
    pub candle_date_time_kst: String,
    pub opening_price: f64,
    pub high_price: f64,
    pub low_price: f64,
    pub trade_price: f64,
    pub timestamp: u64,
    pub candle_acc_trade_price: f64,
    pub candle_acc_trade_volume: f64,
    pub unit: Option<i32>,
}

impl Candle {
    /// upbit_client::Candle을 websocket::Candle로 변환
    pub fn to_websocket_candle(&self) -> crate::websocket::Candle {
        use chrono::{DateTime, Utc};

        crate::websocket::Candle {
            ticker: self.market.clone(),
            open: self.opening_price,
            high: self.high_price,
            low: self.low_price,
            close: self.trade_price,
            volume: self.candle_acc_trade_volume,
            acc_trade_price: self.candle_acc_trade_price,
            start_time: DateTime::from_timestamp_millis(self.timestamp as i64)
                .unwrap_or_else(Utc::now),
            tick_count: 0,
        }
    }
}

impl UpbitClient {
    /// 새로운 Upbit 클라이언트 생성
    pub fn new(access_key: String, secret_key: String) -> Self {
        // 타임아웃 설정: 연결 10초, 읽기 30초
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .connect_timeout(Duration::from_secs(10))
            .build()
            .expect("Failed to create HTTP client");

        // Rate Limiter: 초당 8회 (Upbit 공식 제한: 초당 10회, 여유 있게 설정)
        let rate_limiter = Arc::new(RateLimiter::new(8));

        Self {
            access_key,
            secret_key,
            client,
            base_url: "https://api.upbit.com/v1".to_string(),
            rate_limiter,
        }
    }

    /// API 응답 처리 (에러 체크 포함)
    async fn handle_response<T: serde::de::DeserializeOwned>(
        &self,
        response: reqwest::Response,
    ) -> Result<T> {
        let status = response.status();

        if !status.is_success() {
            let error_text = response.text().await?;

            // Upbit API 에러 형식 파싱 시도
            if let Ok(upbit_error) = serde_json::from_str::<UpbitErrorResponse>(&error_text) {
                anyhow::bail!(
                    "Upbit API 에러 [{}]: {}",
                    upbit_error.error.name,
                    upbit_error.error.message
                );
            }

            // 일반 HTTP 에러
            anyhow::bail!("HTTP {} 에러: {}", status.as_u16(), error_text);
        }

        let data = response.json::<T>().await?;
        Ok(data)
    }

    /// JWT 토큰 생성
    fn create_token(&self, query: Option<&str>) -> Result<String> {
        let nonce = Uuid::new_v4().to_string();
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)?
            .as_millis();

        let mut payload = format!(
            r#"{{"access_key":"{}","nonce":"{}","timestamp":{}}}"#,
            self.access_key, nonce, timestamp
        );

        // 쿼리 문자열이 있으면 추가
        if let Some(q) = query {
            let mut mac = HmacSha512::new_from_slice(self.secret_key.as_bytes())?;
            mac.update(q.as_bytes());
            let query_hash = hex::encode(mac.finalize().into_bytes());

            payload = format!(
                r#"{{"access_key":"{}","nonce":"{}","timestamp":{},"query_hash":"{}","query_hash_alg":"SHA512"}}"#,
                self.access_key, nonce, timestamp, query_hash
            );
        }

        let header = r#"{"alg":"HS256","typ":"JWT"}"#;
        let header_b64 = general_purpose::URL_SAFE_NO_PAD.encode(header);
        let payload_b64 = general_purpose::URL_SAFE_NO_PAD.encode(&payload);

        let message = format!("{}.{}", header_b64, payload_b64);

        let mut mac = HmacSha512::new_from_slice(self.secret_key.as_bytes())?;
        mac.update(message.as_bytes());
        let signature = general_purpose::URL_SAFE_NO_PAD.encode(mac.finalize().into_bytes());

        Ok(format!("{}.{}", message, signature))
    }

    /// 계정 정보 조회 (모든 잔액)
    pub async fn get_balances(&self) -> Result<Vec<Balance>> {
        let token = self.create_token(None)?;
        let url = format!("{}/accounts", self.base_url);

        let response = self
            .client
            .get(&url)
            .header(header::AUTHORIZATION, format!("Bearer {}", token))
            .send()
            .await?;

        let balances: Vec<Balance> = response.json().await?;
        Ok(balances)
    }

    /// 특정 통화 잔액 조회
    pub async fn get_balance(&self, currency: &str) -> Result<f64> {
        let balances = self.get_balances().await?;

        for balance in balances {
            if balance.currency == currency {
                return Ok(balance.balance.parse()?);
            }
        }

        Ok(0.0)
    }

    /// 평균 매수가 조회
    pub async fn get_avg_buy_price(&self, currency: &str) -> Result<Option<f64>> {
        let balances = self.get_balances().await?;

        for balance in balances {
            if balance.currency == currency {
                return Ok(Some(balance.avg_buy_price.parse()?));
            }
        }

        Ok(None)
    }

    /// 시장가 매수
    pub async fn buy_market_order(&self, market: &str, price: f64) -> Result<Order> {
        self.rate_limiter.wait().await;

        let price = price.floor() as i64; // 정수로 변환

        if price < 5000 {
            anyhow::bail!("최소 주문 금액은 5,000원입니다 (입력: {}원)", price);
        }

        let query = format!("market={}&side=bid&ord_type=price&price={}", market, price);
        let token = self.create_token(Some(&query))?;

        let url = format!("{}/orders", self.base_url);

        let price_str = price.to_string();
        let mut params = std::collections::HashMap::new();
        params.insert("market", market);
        params.insert("side", "bid");
        params.insert("ord_type", "price");
        params.insert("price", &price_str);

        log::debug!("매수 주문 요청: {} - {}원", market, price);

        let response = self
            .client
            .post(&url)
            .header(header::AUTHORIZATION, format!("Bearer {}", token))
            .json(&params)
            .send()
            .await?;

        self.handle_response(response).await
    }

    /// 시장가 매도
    pub async fn sell_market_order(&self, market: &str, volume: f64) -> Result<Order> {
        self.rate_limiter.wait().await;

        // 소수점 자리수 조정 (코인별로 다를 수 있지만 일반적으로 8자리)
        let volume_str = format!("{:.8}", volume);

        let query = format!("market={}&side=ask&ord_type=market&volume={}", market, volume_str);
        let token = self.create_token(Some(&query))?;

        let url = format!("{}/orders", self.base_url);

        let mut params = std::collections::HashMap::new();
        params.insert("market", market);
        params.insert("side", "ask");
        params.insert("ord_type", "market");
        params.insert("volume", volume_str.as_str());

        log::debug!("매도 주문 요청: {} - {} 개", market, volume_str);

        let response = self
            .client
            .post(&url)
            .header(header::AUTHORIZATION, format!("Bearer {}", token))
            .json(&params)
            .send()
            .await?;

        self.handle_response(response).await
    }

    /// 주문 상태 조회
    pub async fn get_order(&self, uuid: &str) -> Result<Order> {
        self.rate_limiter.wait().await;

        let query = format!("uuid={}", uuid);
        let token = self.create_token(Some(&query))?;

        let url = format!("{}/order?uuid={}", self.base_url, uuid);

        let response = self
            .client
            .get(&url)
            .header(header::AUTHORIZATION, format!("Bearer {}", token))
            .send()
            .await?;

        self.handle_response(response).await
    }

    /// 마켓 코드 조회
    pub async fn get_market_codes(&self) -> Result<Vec<MarketCode>> {
        let url = format!("{}/market/all", self.base_url);

        let response = self.client.get(&url).send().await?;
        let markets: Vec<MarketCode> = response.json().await?;

        Ok(markets)
    }

    /// KRW 마켓 코드 조회
    pub async fn get_krw_markets(&self) -> Result<Vec<String>> {
        let markets = self.get_market_codes().await?;

        let krw_markets: Vec<String> = markets
            .into_iter()
            .filter(|m| m.market.starts_with("KRW-"))
            .map(|m| m.market)
            .collect();

        Ok(krw_markets)
    }

    /// 현재가 정보 조회
    pub async fn get_ticker(&self, markets: &[String]) -> Result<Vec<Ticker>> {
        let markets_str = markets.join(",");
        let url = format!("{}/ticker?markets={}", self.base_url, markets_str);

        let response = self.client.get(&url).send().await?;
        let tickers: Vec<Ticker> = response.json().await?;

        Ok(tickers)
    }

    /// 단일 마켓 현재가 조회
    pub async fn get_current_price(&self, market: &str) -> Result<f64> {
        self.rate_limiter.wait().await;

        let tickers = self.get_ticker(&[market.to_string()]).await?;

        if let Some(ticker) = tickers.first() {
            Ok(ticker.trade_price)
        } else {
            anyhow::bail!("현재가 조회 실패: {}", market)
        }
    }

    /// 분봉 조회
    pub async fn get_candles_minutes(
        &self,
        market: &str,
        unit: i32,
        count: i32,
    ) -> Result<Vec<Candle>> {
        let url = format!(
            "{}/candles/minutes/{}?market={}&count={}",
            self.base_url, unit, market, count
        );

        let response = self.client.get(&url).send().await?;
        let candles: Vec<Candle> = response.json().await?;

        Ok(candles)
    }
}

