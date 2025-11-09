use anyhow::{Context, Result};
use std::env;

/// 봇 설정 구조체
#[derive(Debug, Clone)]
pub struct Config {
    // API 인증 정보
    pub upbit_access_key: String,
    pub upbit_secret_key: String,

    // 거래 설정
    pub target_profit: f64,      // 목표 수익률 (%)
    pub stop_loss: f64,          // 손절 라인 (%)
    pub trade_amount: f64,       // 거래 금액 비율 (0.0 ~ 1.0)

    // 트레일링 스톱 설정
    pub trailing_stop_enabled: bool,  // 트레일링 스톱 사용 여부
    pub trailing_stop_trigger: f64,   // 트레일링 스톱 활성화 수익률 (%)
    pub trailing_stop_percent: f64,   // 최고가 대비 하락률 (%)

    // 모니터링 설정
    pub candle_interval: u64,    // 캔들 간격 (분)
    pub min_volume_increase: f64, // 최소 거래량 증가율 (배수)
    pub min_price_change: f64,   // 최소 가격 변화율 (%)

    // 리스크 관리
    pub max_position: usize,     // 최대 동시 보유 포지션 수
    pub min_krw_balance: f64,    // 최소 보유 원화 잔액

    // 상수
    pub upbit_fee: f64,          // Upbit 수수료 (0.05%)
}

impl Config {
    /// 환경 변수에서 설정 로드
    pub fn from_env() -> Result<Self> {
        dotenv::dotenv().ok();

        let config = Config {
            upbit_access_key: env::var("UPBIT_ACCESS_KEY")
                .context("UPBIT_ACCESS_KEY가 설정되지 않았습니다")?,
            upbit_secret_key: env::var("UPBIT_SECRET_KEY")
                .context("UPBIT_SECRET_KEY가 설정되지 않았습니다")?,

            target_profit: env::var("TARGET_PROFIT")
                .unwrap_or_else(|_| "3.0".to_string())
                .parse()
                .context("TARGET_PROFIT 파싱 실패")?,

            stop_loss: env::var("STOP_LOSS")
                .unwrap_or_else(|_| "2.0".to_string())
                .parse()
                .context("STOP_LOSS 파싱 실패")?,

            trade_amount: env::var("TRADE_AMOUNT")
                .unwrap_or_else(|_| "0.9".to_string())
                .parse()
                .context("TRADE_AMOUNT 파싱 실패")?,

            trailing_stop_enabled: env::var("TRAILING_STOP_ENABLED")
                .unwrap_or_else(|_| "true".to_string())
                .parse()
                .unwrap_or(true),

            trailing_stop_trigger: env::var("TRAILING_STOP_TRIGGER")
                .unwrap_or_else(|_| "2.0".to_string())
                .parse()
                .context("TRAILING_STOP_TRIGGER 파싱 실패")?,

            trailing_stop_percent: env::var("TRAILING_STOP_PERCENT")
                .unwrap_or_else(|_| "1.0".to_string())
                .parse()
                .context("TRAILING_STOP_PERCENT 파싱 실패")?,

            candle_interval: env::var("CANDLE_INTERVAL")
                .unwrap_or_else(|_| "5".to_string())
                .parse()
                .context("CANDLE_INTERVAL 파싱 실패")?,

            min_volume_increase: env::var("MIN_VOLUME_INCREASE")
                .unwrap_or_else(|_| "1.1".to_string())
                .parse()
                .context("MIN_VOLUME_INCREASE 파싱 실패")?,

            min_price_change: env::var("MIN_PRICE_CHANGE")
                .unwrap_or_else(|_| "0.5".to_string())
                .parse()
                .context("MIN_PRICE_CHANGE 파싱 실패")?,

            max_position: env::var("MAX_POSITION")
                .unwrap_or_else(|_| "1".to_string())
                .parse()
                .context("MAX_POSITION 파싱 실패")?,

            min_krw_balance: env::var("MIN_KRW_BALANCE")
                .unwrap_or_else(|_| "10000".to_string())
                .parse()
                .context("MIN_KRW_BALANCE 파싱 실패")?,

            upbit_fee: 0.0005, // 0.05%
        };

        config.validate()?;
        Ok(config)
    }

    /// 설정 유효성 검증
    fn validate(&self) -> Result<()> {
        // API 키 검증
        if self.upbit_access_key.is_empty() || self.upbit_secret_key.is_empty() {
            anyhow::bail!("API 키가 비어있습니다");
        }

        // 수익률/손절 검증
        if self.target_profit <= 0.0 {
            anyhow::bail!("목표 수익률은 0보다 커야 합니다 (현재: {}%)", self.target_profit);
        }
        if self.target_profit > 100.0 {
            log::warn!("목표 수익률이 매우 높습니다 ({}%). 권장: 1-10%", self.target_profit);
        }

        if self.stop_loss <= 0.0 {
            anyhow::bail!("손절 라인은 0보다 커야 합니다 (현재: {}%)", self.stop_loss);
        }
        if self.stop_loss > 50.0 {
            log::warn!("손절 라인이 매우 높습니다 ({}%). 권장: 1-10%", self.stop_loss);
        }

        // 거래 금액 검증
        if self.trade_amount <= 0.0 || self.trade_amount > 1.0 {
            anyhow::bail!("거래 금액 비율은 0과 1 사이여야 합니다 (현재: {})", self.trade_amount);
        }
        if self.trade_amount > 0.95 {
            log::warn!("거래 금액 비율이 매우 높습니다 ({}). 수수료와 슬리피지를 고려하세요.", self.trade_amount);
        }

        // 트레일링 스톱 검증
        if self.trailing_stop_enabled {
            if self.trailing_stop_trigger <= 0.0 {
                anyhow::bail!("트레일링 스톱 활성화 수익률은 0보다 커야 합니다 (현재: {}%)", self.trailing_stop_trigger);
            }
            if self.trailing_stop_percent <= 0.0 {
                anyhow::bail!("트레일링 스톱 하락률은 0보다 커야 합니다 (현재: {}%)", self.trailing_stop_percent);
            }
            if self.trailing_stop_trigger < self.trailing_stop_percent {
                log::warn!(
                    "트레일링 스톱 활성화 수익률({:.1}%)이 하락률({:.1}%)보다 낮습니다. 즉시 트리거될 수 있습니다.",
                    self.trailing_stop_trigger, self.trailing_stop_percent
                );
            }
        }

        // 캔들 간격 검증
        if self.candle_interval == 0 {
            anyhow::bail!("캔들 간격은 0보다 커야 합니다");
        }
        if ![1, 3, 5, 10, 15, 30, 60, 240].contains(&self.candle_interval) {
            log::warn!("비표준 캔들 간격입니다 ({}분). 권장: 1, 3, 5, 10, 15, 30, 60, 240", self.candle_interval);
        }

        // 거래량/가격 변화 검증
        if self.min_volume_increase <= 1.0 {
            anyhow::bail!("최소 거래량 증가율은 1보다 커야 합니다 (현재: {}배)", self.min_volume_increase);
        }
        if self.min_price_change < 0.0 {
            anyhow::bail!("최소 가격 변화율은 0 이상이어야 합니다 (현재: {}%)", self.min_price_change);
        }

        // 리스크 관리 검증
        if self.max_position == 0 {
            anyhow::bail!("최대 포지션 수는 0보다 커야 합니다");
        }
        if self.max_position > 10 {
            log::warn!("최대 포지션 수가 매우 많습니다 ({}). 리스크 관리에 주의하세요.", self.max_position);
        }

        if self.min_krw_balance < 5000.0 {
            log::warn!("최소 KRW 잔액이 낮습니다 ({:.0}원). Upbit 최소 주문 금액은 5,000원입니다.", self.min_krw_balance);
        }

        Ok(())
    }

    /// 설정 정보 출력
    pub fn display(&self) -> String {
        format!(
            r#"
===== 봇 설정 정보 =====
목표 수익률: {}%
손절 라인: {}%
거래 금액 비율: {:.0}%
캔들 간격: {}분
최소 거래량 증가율: {}배
최소 가격 변화율: {}%
최대 포지션 수: {}
최소 KRW 잔액: {:.0}원
========================
"#,
            self.target_profit,
            self.stop_loss,
            self.trade_amount * 100.0,
            self.candle_interval,
            self.min_volume_increase,
            self.min_price_change,
            self.max_position,
            self.min_krw_balance
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_validation_valid() {
        let config = Config {
            upbit_access_key: "test_key".to_string(),
            upbit_secret_key: "test_secret".to_string(),
            target_profit: 3.0,
            stop_loss: 2.0,
            trade_amount: 0.9,
            trailing_stop_enabled: true,
            trailing_stop_trigger: 2.0,
            trailing_stop_percent: 1.0,
            candle_interval: 5,
            min_volume_increase: 1.5,
            min_price_change: 0.5,
            max_position: 1,
            min_krw_balance: 10000.0,
            upbit_fee: 0.0005,
        };

        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_config_validation_empty_api_key() {
        let config = Config {
            upbit_access_key: "".to_string(),
            upbit_secret_key: "test_secret".to_string(),
            target_profit: 3.0,
            stop_loss: 2.0,
            trade_amount: 0.9,
            trailing_stop_enabled: false,
            trailing_stop_trigger: 2.0,
            trailing_stop_percent: 1.0,
            candle_interval: 5,
            min_volume_increase: 1.5,
            min_price_change: 0.5,
            max_position: 1,
            min_krw_balance: 10000.0,
            upbit_fee: 0.0005,
        };

        assert!(config.validate().is_err());
    }

    #[test]
    fn test_config_validation_invalid_trade_amount() {
        let mut config = Config {
            upbit_access_key: "test_key".to_string(),
            upbit_secret_key: "test_secret".to_string(),
            target_profit: 3.0,
            stop_loss: 2.0,
            trade_amount: 1.5, // Invalid: > 1.0
            trailing_stop_enabled: false,
            trailing_stop_trigger: 2.0,
            trailing_stop_percent: 1.0,
            candle_interval: 5,
            min_volume_increase: 1.5,
            min_price_change: 0.5,
            max_position: 1,
            min_krw_balance: 10000.0,
            upbit_fee: 0.0005,
        };

        assert!(config.validate().is_err());

        config.trade_amount = 0.0; // Invalid: <= 0.0
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_config_validation_invalid_volume_increase() {
        let config = Config {
            upbit_access_key: "test_key".to_string(),
            upbit_secret_key: "test_secret".to_string(),
            target_profit: 3.0,
            stop_loss: 2.0,
            trade_amount: 0.9,
            trailing_stop_enabled: false,
            trailing_stop_trigger: 2.0,
            trailing_stop_percent: 1.0,
            candle_interval: 5,
            min_volume_increase: 0.5, // Invalid: <= 1.0
            min_price_change: 0.5,
            max_position: 1,
            min_krw_balance: 10000.0,
            upbit_fee: 0.0005,
        };

        assert!(config.validate().is_err());
    }
}
