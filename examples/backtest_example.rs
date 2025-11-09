use upbit_trading_bot::{
    analyzer::CandleAnalyzer,
    backtest::BacktestEngine,
    config::Config,
    upbit_client::UpbitClient,
};
use std::sync::Arc;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // 로거 초기화
    env_logger::Builder::from_default_env()
        .filter_level(log::LevelFilter::Info)
        .init();

    println!("===== Upbit 트레이딩 봇 백테스팅 =====\n");

    // 설정 로드
    let config = Arc::new(Config::from_env()?);

    // Upbit 클라이언트 초기화
    let client = Arc::new(UpbitClient::new(
        config.upbit_access_key.clone(),
        config.upbit_secret_key.clone(),
    ));

    // 백테스팅할 마켓 선택
    let market = "KRW-BTC"; // 비트코인으로 테스트
    println!("마켓: {}", market);

    // 과거 데이터 가져오기 (최근 200개 캔들)
    println!("과거 데이터 로딩 중...");
    let candles = client.get_candles(market, 200).await?;
    println!("로드된 캔들 수: {}", candles.len());

    if candles.len() < 20 {
        anyhow::bail!("백테스팅을 위해 최소 20개 이상의 캔들이 필요합니다");
    }

    // 분석기 초기화
    let analyzer = CandleAnalyzer::new(
        config.min_volume_increase,
        config.min_price_change,
    );

    // 백테스팅 엔진 초기화
    let initial_balance = 1_000_000.0; // 초기 자본 100만원
    let mut backtest = BacktestEngine::new(analyzer, (*config).clone(), initial_balance);

    // 백테스팅 실행
    println!("\n백테스팅 실행 중...\n");
    let stats = backtest.run(&candles)?;

    // 결과 출력
    backtest.print_trades();
    backtest.print_stats(&stats);

    Ok(())
}
