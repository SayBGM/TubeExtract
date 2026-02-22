# TubeExtract (yt-downloder)

유튜브 URL을 분석해 영상/오디오를 다운로드할 수 있는 Electron 데스크톱 앱입니다.  
React + TypeScript + Vite 기반 렌더러와 Electron 메인 프로세스로 구성되어 있습니다.

## 주요 기능

- URL 분석 (제목, 채널, 길이, 썸네일, 포맷 옵션 확인)
- 영상/오디오 다운로드 큐 관리
- 다운로드 일시정지/재개/취소
- 완료 항목 파일 열기/삭제
- 설정 관리 (다운로드 경로, 재시도 횟수, 언어)
- 환경 진단/업데이트 확인

## 기술 스택

- Electron
- React 19
- TypeScript
- Vite
- React Query
- React Hook Form
- Zustand
- Tailwind CSS
- Vitest + React Testing Library

## 시작하기

### 1) 의존성 설치

```bash
npm install
```

### 2) 개발 실행

```bash
npm run electron:dev
```

## 스크립트

- `npm run dev`: Vite 개발 서버 실행
- `npm run electron:dev`: Vite + Electron 동시 실행
- `npm run build`: 프론트엔드 빌드
- `npm run electron:build`: 데스크톱 앱 패키징 빌드
- `npm run lint`: ESLint 실행
- `npm run test`: Vitest 실행
- `npm run test:watch`: Vitest watch 모드

## 테스트

```bash
npm run test
```

GitHub Actions에서도 PR/메인 브랜치 푸시 시 단위 테스트를 자동 실행합니다.

## 빌드 산출물

- Electron 빌드 결과물은 `release/` 디렉터리에 생성됩니다.

## 프로젝트 구조 (요약)

```text
electron/                 # Electron 메인 프로세스
src/
  renderer/               # React 렌더러 앱
    domains/              # 도메인별 기능 (setup, queue, settings)
    components/           # 공용 컴포넌트
    lib/                  # Electron 브리지/유틸
    queries/              # React Query 키/옵션
    store/                # Zustand 스토어
  test/                   # 테스트 유틸/목
.github/workflows/        # CI 워크플로우
```
