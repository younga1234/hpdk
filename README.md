# HPDK - 현대적인 비즈니스 웹사이트

전문적이고 현대적인 반응형 웹사이트 템플릿입니다.

## 특징

- ✨ 현대적이고 깔끔한 디자인
- 📱 완벽한 반응형 레이아웃 (모바일, 태블릿, 데스크톱)
- 🎨 그라디언트 및 애니메이션 효과
- ⚡ 빠른 로딩 속도
- 🔍 SEO 최적화
- ♿ 접근성 고려

## 페이지 구성

1. **홈 (Hero Section)** - 매력적인 첫인상
2. **소개 (About)** - 회사 소개 및 통계
3. **서비스 (Services)** - 제공하는 서비스 목록
4. **포트폴리오 (Portfolio)** - 작업 사례
5. **연락처 (Contact)** - 연락 폼 및 정보

## 기술 스택

- HTML5
- CSS3 (CSS Grid, Flexbox, Animations)
- Vanilla JavaScript (ES6+)

## 주요 기능

### 1. 반응형 네비게이션
- 데스크톱: 가로 메뉴
- 모바일: 햄버거 메뉴

### 2. 부드러운 스크롤
- 섹션 간 부드러운 이동
- 스크롤 진행 표시기

### 3. 애니메이션
- 스크롤 시 요소 나타나기
- 숫자 카운터 애니메이션
- 호버 효과

### 4. 연락 폼
- 유효성 검사
- 성공/오류 메시지
- 이메일 형식 검증

## 설치 및 사용

### 1. 로컬 환경에서 실행

```bash
# 저장소 클론
git clone https://github.com/yourusername/hpdk.git

# 디렉토리 이동
cd hpdk

# 브라우저로 index.html 열기
# 또는 Live Server 사용
```

### 2. 웹 서버에 배포

파일들을 웹 서버의 루트 디렉토리에 업로드하면 됩니다.

```
/
├── index.html
├── styles.css
├── script.js
└── README.md
```

## 커스터마이징

### 색상 변경

`styles.css` 파일의 CSS 변수를 수정하세요:

```css
:root {
    --primary-color: #6366f1;      /* 메인 색상 */
    --secondary-color: #8b5cf6;    /* 보조 색상 */
    --dark-color: #1f2937;         /* 어두운 색상 */
    --light-color: #f9fafb;        /* 밝은 색상 */
    --text-color: #374151;         /* 텍스트 색상 */
}
```

### 내용 변경

`index.html` 파일에서 텍스트 내용을 수정하세요:

- 회사명, 로고
- 서비스 설명
- 포트폴리오 항목
- 연락처 정보

### 이미지 추가

포트폴리오 섹션에 실제 이미지를 추가하려면:

```html
<div class="portfolio-image">
    <img src="your-image.jpg" alt="프로젝트 이름">
</div>
```

## 브라우저 지원

- Chrome (최신)
- Firefox (최신)
- Safari (최신)
- Edge (최신)

## 성능 최적화

- CSS 애니메이션 하드웨어 가속
- 이미지 지연 로딩 가능
- 최소화된 CSS/JS (프로덕션 환경)

## 라이선스

이 프로젝트는 MIT 라이선스를 따릅니다.

## 기여

기여는 언제나 환영합니다! Pull Request를 보내주세요.

## 문의

프로젝트에 대한 질문이나 제안사항이 있으시면 이슈를 등록해주세요.

---

Made with ❤️ by HPDK Team