---
id: SPEC-UPDATE-001
version: 0.1.0
status: completed
created: 2026-03-02
updated: 2026-03-02
author: backgwangmin
priority: medium
domain: UPDATE
methodology: ddd
---

# SPEC-UPDATE-001: 자동 업데이트 및 릴리즈 시스템 구현

## Environment (현재 환경)

### 프로젝트 개요

- **프로젝트**: TubeExtract (yt-downloader) — Tauri 2.0 데스크탑 앱
- **스택**: TypeScript (프론트엔드) + Rust (백엔드 `src-tauri/src/`)
- **플랫폼**: macOS, Windows (Linux 빌드 없음)

### 현재 상태 (AS-IS)

| 컴포넌트 | 현재 상태 | 문제점 |
|---------|----------|--------|
| `check_update()` (diagnostics.rs:224-232) | 항상 `hasUpdate: false` 반환하는 스텁 | 실제 버전 확인 불가 |
| `tauri-plugin-updater` | Cargo.toml에 미포함 | 인앱 업데이트 기능 없음 |
| Cargo.toml 버전 (0.1.0) | CI에서 동기화 안 됨 | package.json (0.0.0)과 불일치 |
| 릴리즈 프로세스 | 수동 `git tag + git push` 필요 | 자동화 없음 |
| 프론트엔드 UI | SettingsUpdateSection.tsx 존재 | 백엔드 스텁으로 인해 동작 불가 |

### 관련 파일

| 파일 | 라인 | 용도 |
|------|------|------|
| `src-tauri/src/diagnostics.rs` | 224-232 | `check_update()` 스텁 |
| `src-tauri/src/lib.rs` | 102 | 커맨드 등록 |
| `src-tauri/Cargo.toml` | 23 | Tauri 피처 (updater 없음) |
| `src-tauri/tauri.conf.json` | 1-41 | 앱 설정 (updater 섹션 없음) |
| `.github/workflows/release.yml` | 1-136 | 릴리즈 파이프라인 |
| `src/renderer/lib/desktopClient.ts` | 294-301 | `checkUpdate()` IPC |
| `src/renderer/domains/settings/SettingsPage.tsx` | 75-97 | 업데이트 뮤테이션 |
| `src/renderer/domains/settings/components/SettingsUpdateSection.tsx` | 1-40 | 업데이트 UI |

### CI/CD 현황

- **트리거**: `git tag v*` 푸시 시 실행
- **버전 동기화**: `package.json` + `tauri.conf.json` 동기화 (Cargo.toml 누락)
- **빌드 매트릭스**: macOS, Windows
- **npm release 스크립트**: 존재하지 않음

---

## Assumptions (가정)

1. **GitHub Releases API 가용성**: `https://api.github.com/repos/{owner}/{repo}/releases/latest` 엔드포인트가 공개 접근 가능하며 안정적으로 응답한다고 가정한다.

2. **브라우저 기반 다운로드 흐름 수용**: `tauri-plugin-updater` 없이 브라우저로 GitHub Release 페이지를 열어 사용자가 직접 다운로드하는 방식이 허용된다고 가정한다. 프론트엔드 UI가 이미 이 방식을 구현하고 있다.

3. **Cargo.toml 버전 독립 관리 불필요**: Cargo.toml 버전을 package.json / tauri.conf.json과 동기화하는 것이 일관성 유지에 충분하며, Rust crate 버전의 독립적 관리는 현재 범위 밖이다.

4. **Node.js 환경 가용**: 릴리즈 스크립트(`scripts/release.js`)는 Node.js 런타임이 개발 환경에 설치되어 있다고 가정한다. 추가 외부 의존성은 최소화한다.

5. **semver 표준 준수**: 버전 형식은 `MAJOR.MINOR.PATCH` (예: `0.2.6`) 표준을 따르며, git 태그는 `v` 접두사를 가진다 (예: `v0.2.6`).

---

## Requirements (요구사항)

### REQ-001: check_update() GitHub API 연동

**[Ubiquitous]** 시스템은 항상 `check_update()` 호출 시 실제 GitHub Releases API를 통해 최신 버전을 확인해야 한다.

**[Event-Driven]** WHEN 사용자가 업데이트 확인 버튼을 누를 때 THEN 시스템은 GitHub Releases API에 HTTP GET 요청을 보내고 현재 앱 버전과 비교하여 결과를 반환해야 한다.

**[State-Driven]** IF GitHub API 요청이 실패하거나 타임아웃이 발생하면 THEN 시스템은 `hasUpdate: false`와 적절한 오류 메시지를 반환해야 한다.

**[Unwanted]** 시스템은 GitHub API 요청 실패 시 앱을 크래시시키거나 패닉 상태로 전환하지 않아야 한다.

**응답 형식**:
```json
{
  "hasUpdate": true,
  "latestVersion": "0.3.0",
  "url": "https://github.com/owner/repo/releases/tag/v0.3.0"
}
```

### REQ-002: 릴리즈 커맨드 개발 (npm run release)

**[Ubiquitous]** 시스템은 항상 `npm run release` 명령어 실행 전에 git working tree가 clean한지 검증해야 한다.

**[Event-Driven]** WHEN `npm run release -- --patch|--minor|--major|--version X.Y.Z`가 실행될 때 THEN 시스템은 semver 규칙에 따라 버전을 계산하고 세 개의 파일(package.json, tauri.conf.json, Cargo.toml)의 버전을 업데이트해야 한다.

**[Event-Driven]** WHEN 버전 파일 업데이트가 완료되면 THEN 시스템은 `chore(release): v{version}` 커밋을 생성하고, `v{version}` 태그를 붙이고, 커밋과 태그를 원격 저장소에 푸시해야 한다.

**[Unwanted]** IF git working tree가 dirty (uncommitted changes 존재) 하면 THEN 시스템은 즉시 오류를 출력하고 릴리즈 프로세스를 중단해야 한다.

**[Unwanted]** 시스템은 `--patch`, `--minor`, `--major`, `--version` 중 어느 플래그도 없이 실행될 때 릴리즈를 진행하지 않아야 한다.

**지원 명령어**:
```bash
npm run release -- --patch    # 0.2.5 → 0.2.6
npm run release -- --minor    # 0.2.5 → 0.3.0
npm run release -- --major    # 0.2.5 → 1.0.0
npm run release -- --version 1.0.0  # 명시적 버전
```

### REQ-003: CI/CD Cargo.toml 버전 동기화

**[Event-Driven]** WHEN GitHub Actions release.yml이 `v*` 태그 트리거로 실행될 때 THEN 시스템은 git 태그에서 추출한 버전을 `src-tauri/Cargo.toml`의 `[package].version` 필드에도 동기화해야 한다.

**[Ubiquitous]** 시스템은 항상 릴리즈 빌드 시 package.json, tauri.conf.json, Cargo.toml 세 파일의 버전이 동일하도록 보장해야 한다.

### REQ-004: 동작 보존 검증 (DDD PRESERVE)

**[Ubiquitous]** 시스템은 항상 `check_update()` 변경 전에 현재 스텁 동작(항상 `hasUpdate: false` 반환)을 특성화 테스트로 문서화해야 한다.

**[State-Driven]** IF 기존 프론트엔드 코드(`desktopClient.ts`, `SettingsPage.tsx`, `SettingsUpdateSection.tsx`)가 정상 동작하고 있다면 THEN 백엔드 변경 후에도 동일한 프론트엔드 인터페이스가 유지되어야 한다.

---

## Specifications (명세)

### SPEC-001: Rust check_update() 구현

**대상 파일**: `src-tauri/src/diagnostics.rs`

**GitHub 레포지토리 정보 소스**: `tauri::app::App::package_info()` 또는 하드코딩된 GitHub URL 상수로부터 owner/repo 추출

**구현 세부사항**:

1. `reqwest` crate를 Cargo.toml에 추가 (TLS 피처 포함)
2. GitHub Releases API 엔드포인트: `https://api.github.com/repos/{owner}/{repo}/releases/latest`
3. User-Agent 헤더 필수: GitHub API 정책 준수
4. 현재 앱 버전: `app.package_info().version` 사용
5. 버전 비교: semver 파싱 후 비교 (`semver` crate 또는 수동 파싱)
6. 타임아웃: 10초 이내
7. 반환 구조체:

```rust
// 반환 형식 유지
{
    "hasUpdate": bool,
    "latestVersion": String | null,
    "url": String | null
}
```

**오류 처리**:
- 네트워크 오류 → `hasUpdate: false`, 오류 메시지 로그
- JSON 파싱 오류 → `hasUpdate: false`
- HTTP 4xx/5xx → `hasUpdate: false`

### SPEC-002: 릴리즈 스크립트 구현

**대상 파일**: `scripts/release.js` (신규 생성)

**의존성**: Node.js 내장 모듈만 사용 (`fs`, `path`, `child_process`, `readline`)

**처리 흐름**:

```
1. 인자 파싱 (--patch / --minor / --major / --version X.Y.Z)
2. git status 확인 → dirty이면 오류 출력 후 종료 (exit 1)
3. package.json에서 현재 버전 읽기
4. 새 버전 계산 (semver bump 또는 명시적 버전)
5. 세 파일 버전 업데이트:
   - package.json: "version" 필드
   - src-tauri/tauri.conf.json: "version" 필드
   - src-tauri/Cargo.toml: [package] version = "X.Y.Z"
6. git add -A
7. git commit -m "chore(release): v{version}"
8. git tag v{version}
9. git push && git push --tags
10. 성공 메시지 출력
```

**package.json 스크립트 등록**:
```json
{
  "scripts": {
    "release": "node scripts/release.js"
  }
}
```

### SPEC-003: release.yml Cargo.toml 버전 동기화

**대상 파일**: `.github/workflows/release.yml`

**현재 버전 동기화 로직** (lines 37-51):
- package.json 버전 업데이트: `npm version $VERSION --no-git-tag-version`
- tauri.conf.json 버전 업데이트: `jq` 사용

**추가할 동기화 로직**:
- `sed -i` 또는 `cargo set-version` 등을 사용하여 Cargo.toml의 `version = "..."` 라인 업데이트
- 위치: 기존 버전 동기화 스텝 바로 뒤에 추가
- 변수 재사용: 이미 추출된 `VERSION` 변수 활용

---

## Traceability (추적성)

| 요구사항 | 관련 파일 | 수락 기준 |
|---------|----------|----------|
| REQ-001 | diagnostics.rs | GitHub API 실제 호출, hasUpdate 정확한 반환 |
| REQ-002 | scripts/release.js, package.json | 세 파일 버전 일치, 태그 푸시 성공 |
| REQ-003 | .github/workflows/release.yml | Cargo.toml 버전이 git 태그와 일치 |
| REQ-004 | diagnostics.rs (특성화 테스트) | 기존 인터페이스 유지, 회귀 없음 |

---

## 구현 완료 노트 (Implementation Notes) — 2026-03-02

**상태**: COMPLETED

### 계획 대비 실제 구현

모든 4개 REQ 요구사항이 계획대로 구현되었습니다.

**편차 없음:**
- **reqwest 의존성**: Cargo.toml에 이미 존재하여 추가 불필요
- **check_update() 서명 변경**: `()` → `(app: tauri::AppHandle)` — Tauri가 자동 주입하므로 프론트엔드 영향 없음
- **테스트**: 총 99개 (기존 95개 + 신규 특성화 4개), 모두 통과

### 핵심 설계 결정

1. **GitHub Releases API 직접 호출**: `tauri-plugin-updater` 없이 `reqwest`로 직접 구현하여 브라우저 기반 다운로드 흐름 유지
2. **안전한 오류 처리**: 네트워크 실패, 타임아웃, JSON 파싱 오류 모두 `hasUpdate: false`로 폴백 (패닉 없음)
3. **릴리즈 스크립트**: Node.js 내장 모듈만 사용 (외부 의존성 없음)
4. **Cargo.toml regex**: `^version\s*=\s*"[^"]*"`로 `[package]` 섹션 버전만 교체 (의존성 버전 보존)

### 변경 파일 요약

| 파일 | 변경 내용 | 상태 |
|------|---------|------|
| src-tauri/src/diagnostics.rs | check_update() 구현 + 특성화 4개 추가 | ✓ 커밋됨 |
| scripts/release.js | 신규 릴리즈 스크립트 생성 | ✓ 커밋됨 |
| package.json | "release" 스크립트 등록 | ✓ 커밋됨 |
| .github/workflows/release.yml | Cargo.toml 버전 동기화 스텝 추가 | ✓ 커밋됨 |

## TAG Block

```
SPEC-ID: SPEC-UPDATE-001
DOMAIN: UPDATE
STATUS: completed
CREATED: 2026-03-02
COMPLETED: 2026-03-02
METHODOLOGY: ddd
SCOPE: [diagnostics.rs, release.yml, scripts/release.js, package.json]
DEPENDENCIES: [reqwest (이미 존재)]
```
