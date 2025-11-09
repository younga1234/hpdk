# Upbit 자동매매봇 (Rust)

Upbit 암호화폐 자동매매 시스템 - Rust로 구현한 고성능 트레이딩 봇

## 주요 기능

### 실시간 모니터링
- WebSocket API를 통한 실시간 데이터 수신
- 전체 KRW 마켓 코인 동시 모니터링
- 5분봉 캔들 데이터 실시간 집계 및 분석

### 매수 전략 (개선된 버전)
다음 조건을 **모두** 만족할 때 매수:

1. **거래량 증가**: 이전 5분봉 대비 거래량 증가
2. **가격 상승**: 현재 캔들이 양봉이고 최소 0.5% 이상 상승
3. **모멘텀 증가**: 현재 가격 상승률이 이전보다 증가
4. **평균 거래량 대비**: 평균 거래량 대비 1.5배 이상 증가
5. **과매수 필터링**: 단기간 10% 이상 급등한 종목 제외 (고점 매수 방지)
6. **반등 패턴 선호**: 하락 후 반등하는 패턴에 가산점

### 매도 전략
- **익절**: 목표 수익률 달성 시 (기본값: +3%)
- **손절**: 손절 라인 도달 시 (기본값: -2%)
- 실시간 포지션 모니터링 (10초 간격)

### 리스크 관리
- 최대 동시 보유 포지션: 1개
- 최소 KRW 잔액 유지
- 수수료 자동 반영 (0.05%)

## 시스템 요구사항

- Rust 1.70 이상
- 인터넷 연결
- Upbit API 키 (거래 및 조회 권한 필요)

## 설치 방법

### 1. Rust 설치
```bash
# Linux/macOS
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Windows
# https://rustup.rs/ 에서 설치 프로그램 다운로드
```

### 2. 환경 변수 설정
`.env.example` 파일을 `.env`로 복사하고 API 키 입력:

```bash
cp .env.example .env
```

`.env` 파일 편집:
```bash
# Upbit API Keys (필수)
UPBIT_ACCESS_KEY=your_access_key_here
UPBIT_SECRET_KEY=your_secret_key_here

# Trading Configuration (선택)
TARGET_PROFIT=3.0      # 목표 수익률 (%)
STOP_LOSS=2.0          # 손절 라인 (%)
TRADE_AMOUNT=0.9       # 거래 금액 비율 (90%)
```

### 3. 빌드 및 실행

```bash
# 개발 모드
cargo build
cargo run

# 릴리스 모드 (최적화)
cargo build --release
./target/release/upbit-trading-bot
```

## Upbit API 키 발급

1. [Upbit](https://upbit.com/) 로그인
2. 마이페이지 → Open API 관리
3. API 키 발급 (자산 조회, 주문 조회, 주문하기 권한 활성화)
4. Access Key와 Secret Key를 `.env` 파일에 입력

⚠️ **보안 주의**: API 키는 절대 공개하지 마세요. 출금 권한은 절대 활성화하지 마세요.

## 프로젝트 구조

```
upbit-trading-bot/
├── src/
│   ├── main.rs              # 메인 로직
│   ├── config.rs            # 설정 관리
│   ├── upbit_client.rs      # Upbit API 클라이언트
│   ├── websocket.rs         # WebSocket 및 캔들 집계
│   ├── analyzer.rs          # 캔들 데이터 분석
│   └── trading.rs           # 거래 전략 실행
├── Cargo.toml               # Rust 의존성 설정
├── .env                     # 환경 변수 (API 키)
└── README.md                # 프로젝트 문서
```

## ⚠️ 투자 위험 경고

1. **암호화폐는 변동성이 매우 높습니다** - 손실 가능성을 항상 인지하세요
2. **이 봇은 수익을 보장하지 않습니다** - 소액으로 먼저 테스트하세요
3. **API 키 보안** - 유출 시 자산을 잃을 수 있습니다
4. **법적 책임** - 모든 손실은 사용자 책임입니다

## 라이선스

MIT License

---

**면책 조항**: 이 소프트웨어는 교육 목적으로 제공됩니다. 실제 투자 결정은 본인의 책임입니다.
