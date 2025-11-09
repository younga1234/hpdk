# 트레이딩 전략 문서

## 📊 전략 개요

Upbit 자동매매봇은 **급등 코인 조기 포착** 및 **기술적 지표 기반 진입**을 핵심으로 하는 알고리즘 트레이딩 시스템입니다.

## 🎯 매수 전략

### 1. Volume Surge Detection (거래량 급증 감지)

**학술 근거**: arXiv "Detecting Crypto Pump-and-Dump Schemes"
- EWMA (지수 가중 이동평균) 기반 거래량 분석
- 평균 대비 **1.5~2배** 이상 거래량 증가 시 신호
- Pump & Dump 패턴 조기 감지

```rust
// 거래량 비율 체크
let volume_vs_avg = current_volume / average_volume;
if volume_vs_avg >= 1.5 {
    // 급등 신호
}
```

### 2. 기술적 지표 통합 (Multi-Indicator Approach)

#### RSI (Relative Strength Index)
- **과매도 구간 (< 30)**: 매수 기회
- **과매수 구간 (> 70)**: 매수 회피
- 역추세 신호로 활용

#### MACD (Moving Average Convergence Divergence)
- **골든크로스**: MACD 선이 Signal 선 상향 돌파
- **상승 모멘텀**: MACD > 0

#### Bollinger Bands
- **하단 돌파 후 반등**: 저점 매수 기회
- **중간선 돌파**: 상승 확인

#### 이동평균선 크로스
- **단기 MA (5) > 장기 MA (20)**: 골든크로스

### 3. 지표 점수 시스템

최소 **2개 이상**의 긍정적 신호 필요:

```
지표 점수 계산:
- MACD 상승/골든크로스: +1점
- Bollinger Band 하단 신호: +1점
- MA 골든크로스: +1점
- RSI 과매도: +1점

매수 조건: 점수 >= 2 OR 반등 패턴 OR 거래량 2배 이상
```

### 4. 과매수 필터링

**고점 매수 방지**:
- 단기간(5캔들) **10% 이상** 급등 시 매수 제외
- 이미 pump가 완료된 코인 회피

### 5. 반등 패턴 선호

**추세 반전 포착**:
- 2개 연속 음봉 → 강한 양봉
- 하락 후 반등하는 구간에서 진입

## 💰 매도 전략

### 1. 고정 익절/손절

```
익절: +3.0% (기본값)
손절: -2.0% (기본값)
```

### 2. 트레일링 스톱

**활성화 조건**: 수익률 **+2.0%** 도달 시

**작동 방식**:
1. 수익 +2% 도달 → 트레일링 스톱 활성화
2. 최고가 지속 갱신
3. 최고가 대비 **-1.0%** 하락 시 자동 매도

**장점**:
- 수익 보호
- 추가 상승 시 이익 극대화

```
예시:
매수가: 10,000원
현재가: 10,200원 (+2.0%) → 트레일링 스톱 활성화
최고가: 10,300원
매도 신호: 10,197원 (최고가 대비 -1.0%)
최종 수익: +1.97%
```

## 🛡️ 리스크 관리

### 1. 포지션 사이징

**업계 표준** (Babypips.com, BitMEX 블로그):
- 거래당 리스크: **1-3%**
- 초보자 권장: **1%**

**공식**:
```
포지션 크기 = (계좌 * 리스크%) / (진입가 - 손절가)
```

**현재 설정**:
```
거래 금액: 계좌의 90%
최소 잔액: 10,000원
최대 포지션: 1개
```

### 2. 손절매 (Stop Loss)

**필수 원칙**:
- 모든 거래에 손절매 설정
- 최대 손실: 계좌의 **-2%**
- 감정적 판단 배제

### 3. 자금 보존

> "자본 보존은 성공적인 투기의 첫 번째 우선순위" - CME Group

- 과도한 레버리지 금지
- 분산 투자 (현재: 1개 포지션만 허용)
- 수수료 고려 (Upbit: 0.05%)

## 📈 성능 최적화

### 1. 백테스팅

**측정 지표**:
- 승률 (Win Rate)
- 평균 수익 (Average Profit)
- 최대 손실 (Max Drawdown)
- 평균 보유 시간

**실행 방법**:
```bash
cargo run --example backtest_example
```

### 2. 실시간 모니터링

- 10초 간격 포지션 체크
- 60초 간격 잔액 업데이트
- GUI 대시보드 실시간 표시

## 🔬 학술 근거 및 참고 자료

### Pump Detection
1. **"Detecting Crypto Pump-and-Dump Schemes"** (arXiv)
   - EWMA + 임계값 기반 모델
   - 거래량 400% + 가격 90% 임계값

2. **"Machine Learning-Based Detection"** (arXiv)
   - 30초 간격 ultra-fine 분석
   - 1시간 전 pump 패턴 예측

### Risk Management
1. **Babypips.com**: "3 Key Concepts of Risk Management"
   - 1-3% 리스크 원칙
   - 포지션 사이징 공식

2. **BitMEX Blog**: "Mastering Risk Management in Crypto"
   - 자본 보존 우선
   - 손절매 필수

3. **CME Group**: "Position and Risk Management"
   - 계약 수량 조절
   - 증거금 요건 초과 금지

### Technical Analysis
1. **Medium**: "RSI, MACD, Bollinger Bands Hybrid Strategy"
   - 다중 지표 통합 방법
   - 신호 점수 시스템

2. **QuantifiedStrategies.com**: "MACD and Bollinger Bands Strategy"
   - 78% 승률 백테스트 결과
   - 조합 전략 효과

## 🚀 향후 개선 방향

### 1. Order Book 분석
- 매수/매도 벽 분석
- 실제 수요 vs 인위적 pump 구분

### 2. 더 빠른 진입
- 현재: 5분봉
- 개선: 1분봉 또는 30초 tick

### 3. 다중 포지션
- 현재: 1개
- 개선: 2~3개 (분산 투자)

### 4. AI/ML 통합
- 패턴 인식 강화
- 실시간 학습 모델

## ⚠️ 면책 조항

- 이 전략은 **수익을 보장하지 않습니다**
- 암호화폐는 **극심한 변동성**이 있습니다
- **소액**으로 먼저 테스트하세요
- 모든 손실은 **사용자 책임**입니다

---

**참고 문헌**:
- https://arxiv.org/html/2503.08692v1
- https://www.babypips.com/crypto/learn/risk-management-concepts-crypto-traders-should-know
- https://blog.bitmex.com/crypto-risk-management/
- https://www.quantifiedstrategies.com/macd-and-bollinger-bands-strategy/
- https://docs.upbit.com/kr (Upbit API 공식 문서)
