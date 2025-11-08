// DOM 요소들
const hamburger = document.querySelector('.hamburger');
const navMenu = document.querySelector('.nav-menu');
const navLinks = document.querySelectorAll('.nav-link');
const navbar = document.querySelector('.navbar');
const contactForm = document.querySelector('.contact-form');

// 모바일 메뉴 토글
hamburger.addEventListener('click', () => {
    hamburger.classList.toggle('active');
    navMenu.classList.toggle('active');
});

// 네비게이션 링크 클릭 시 메뉴 닫기
navLinks.forEach(link => {
    link.addEventListener('click', () => {
        hamburger.classList.remove('active');
        navMenu.classList.remove('active');
    });
});

// 스크롤 시 네비게이션 바 스타일 변경
window.addEventListener('scroll', () => {
    if (window.scrollY > 100) {
        navbar.style.boxShadow = '0 4px 20px rgba(0, 0, 0, 0.1)';
        navbar.style.padding = '0.5rem 0';
    } else {
        navbar.style.boxShadow = '0 2px 10px rgba(0, 0, 0, 0.1)';
        navbar.style.padding = '1rem 0';
    }
});

// 부드러운 스크롤
navLinks.forEach(link => {
    link.addEventListener('click', (e) => {
        e.preventDefault();
        const targetId = link.getAttribute('href');
        const targetSection = document.querySelector(targetId);

        if (targetSection) {
            const offsetTop = targetSection.offsetTop - 80;
            window.scrollTo({
                top: offsetTop,
                behavior: 'smooth'
            });
        }
    });
});

// 스크롤 애니메이션
const observerOptions = {
    threshold: 0.1,
    rootMargin: '0px 0px -100px 0px'
};

const observer = new IntersectionObserver((entries) => {
    entries.forEach(entry => {
        if (entry.isIntersecting) {
            entry.target.style.opacity = '1';
            entry.target.style.transform = 'translateY(0)';
        }
    });
}, observerOptions);

// 애니메이션 대상 요소들
const animatedElements = document.querySelectorAll('.service-card, .portfolio-item, .stat-item');
animatedElements.forEach(el => {
    el.style.opacity = '0';
    el.style.transform = 'translateY(30px)';
    el.style.transition = 'opacity 0.6s ease, transform 0.6s ease';
    observer.observe(el);
});

// 연락처 폼 제출 처리
contactForm.addEventListener('submit', (e) => {
    e.preventDefault();

    // 폼 데이터 가져오기
    const name = document.getElementById('name').value;
    const email = document.getElementById('email').value;
    const subject = document.getElementById('subject').value;
    const message = document.getElementById('message').value;

    // 유효성 검사
    if (!name || !email || !subject || !message) {
        showMessage('모든 필드를 입력해주세요.', 'error');
        return;
    }

    // 이메일 유효성 검사
    const emailRegex = /^[^\s@]+@[^\s@]+\.[^\s@]+$/;
    if (!emailRegex.test(email)) {
        showMessage('올바른 이메일 주소를 입력해주세요.', 'error');
        return;
    }

    // 성공 메시지 (실제로는 서버로 전송)
    showMessage('메시지가 성공적으로 전송되었습니다!', 'success');

    // 폼 초기화
    contactForm.reset();
});

// 메시지 표시 함수
function showMessage(text, type) {
    // 기존 메시지 제거
    const existingMessage = document.querySelector('.form-message');
    if (existingMessage) {
        existingMessage.remove();
    }

    // 새 메시지 생성
    const messageDiv = document.createElement('div');
    messageDiv.className = `form-message ${type}`;
    messageDiv.textContent = text;
    messageDiv.style.cssText = `
        padding: 1rem;
        margin-top: 1rem;
        border-radius: 10px;
        text-align: center;
        font-weight: 500;
        animation: slideIn 0.3s ease;
        ${type === 'success'
            ? 'background: #d1fae5; color: #065f46; border: 2px solid #10b981;'
            : 'background: #fee2e2; color: #991b1b; border: 2px solid #ef4444;'}
    `;

    contactForm.appendChild(messageDiv);

    // 3초 후 메시지 제거
    setTimeout(() => {
        messageDiv.style.animation = 'slideOut 0.3s ease';
        setTimeout(() => messageDiv.remove(), 300);
    }, 3000);
}

// 스크롤 진행 표시기 (선택사항)
function createScrollProgress() {
    const progressBar = document.createElement('div');
    progressBar.id = 'scroll-progress';
    progressBar.style.cssText = `
        position: fixed;
        top: 0;
        left: 0;
        height: 4px;
        background: linear-gradient(135deg, #6366f1, #8b5cf6);
        width: 0%;
        z-index: 9999;
        transition: width 0.1s ease;
    `;
    document.body.appendChild(progressBar);

    window.addEventListener('scroll', () => {
        const scrollHeight = document.documentElement.scrollHeight - window.innerHeight;
        const scrolled = (window.scrollY / scrollHeight) * 100;
        progressBar.style.width = scrolled + '%';
    });
}

createScrollProgress();

// 애니메이션 키프레임 추가
const style = document.createElement('style');
style.textContent = `
    @keyframes slideIn {
        from {
            opacity: 0;
            transform: translateY(-20px);
        }
        to {
            opacity: 1;
            transform: translateY(0);
        }
    }

    @keyframes slideOut {
        from {
            opacity: 1;
            transform: translateY(0);
        }
        to {
            opacity: 0;
            transform: translateY(-20px);
        }
    }
`;
document.head.appendChild(style);

// 숫자 카운트 애니메이션
function animateCounter(element, target) {
    let current = 0;
    const increment = target / 50;
    const timer = setInterval(() => {
        current += increment;
        if (current >= target) {
            element.textContent = target + '+';
            clearInterval(timer);
        } else {
            element.textContent = Math.floor(current) + '+';
        }
    }, 30);
}

// 통계 섹션이 보일 때 카운터 애니메이션 시작
const statsObserver = new IntersectionObserver((entries) => {
    entries.forEach(entry => {
        if (entry.isIntersecting && !entry.target.classList.contains('counted')) {
            const statItems = entry.target.querySelectorAll('.stat-item h4');
            statItems.forEach(item => {
                const text = item.textContent;
                const number = parseInt(text.replace('+', ''));
                item.textContent = '0+';
                animateCounter(item, number);
            });
            entry.target.classList.add('counted');
        }
    });
}, { threshold: 0.5 });

const aboutSection = document.querySelector('.about');
if (aboutSection) {
    statsObserver.observe(aboutSection);
}

// 페이지 로드 완료 시
window.addEventListener('load', () => {
    // 로딩 애니메이션 등 추가 가능
    console.log('HPDK 웹사이트가 로드되었습니다.');
});

// 외부 클릭 시 모바일 메뉴 닫기
document.addEventListener('click', (e) => {
    if (!hamburger.contains(e.target) && !navMenu.contains(e.target)) {
        hamburger.classList.remove('active');
        navMenu.classList.remove('active');
    }
});
