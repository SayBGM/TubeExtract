---
spec-id: SPEC-UPDATE-001
type: acceptance
created: 2026-03-02
updated: 2026-03-02
methodology: ddd
---

# Acceptance Criteria: SPEC-UPDATE-001 — 자동 업데이트 및 릴리즈 시스템 구현

## 개요

본 문서는 SPEC-UPDATE-001의 수락 기준을 Given-When-Then 형식으로 정의한다. 각 시나리오는 독립적으로 검증 가능하며, 모든 시나리오가 통과해야 구현이 완료된 것으로 간주한다.

---

## Scenario 1: 최신 버전이 GitHub에 존재하는 경우 업데이트 감지

**관련 요구사항**: REQ-001

**Given**
- TubeExtract 앱이 버전 `0.2.0`으로 실행 중이다
- GitHub Releases API(`/repos/{owner}/{repo}/releases/latest`)가 `tag_name: "v0.3.0"`과 `html_url: "https://github.com/owner/repo/releases/tag/v0.3.0"`을 포함한 응답을 반환한다

**When**
- 사용자가 설정 화면의 "업데이트 확인" 버튼을 클릭한다
- (또는) `check_update()` Tauri 커맨드가 직접 호출된다

**Then**
- `check_update()`는 다음을 반환해야 한다:
  ```json
  {
    "hasUpdate": true,
    "latestVersion": "0.3.0",
    "url": "https://github.com/owner/repo/releases/tag/v0.3.0"
  }
  ```
- `hasUpdate`가 `true`이므로 프론트엔드 UI는 업데이트 알림을 표시한다
- "다운로드" 버튼 클릭 시 `url`이 브라우저에서 열린다

**검증 방법**
- 단위 테스트: mock HTTP 서버로 API 응답 시뮬레이션
- 통합 테스트: 실제 GitHub API 응답으로 버전 비교 로직 검증

---

## Scenario 2: 이미 최신 버전인 경우 업데이트 없음

**관련 요구사항**: REQ-001

**Given**
- TubeExtract 앱이 버전 `0.3.0`으로 실행 중이다
- GitHub Releases API가 `tag_name: "v0.3.0"`을 포함한 응답을 반환한다 (현재 버전과 동일)

**When**
- `check_update()` Tauri 커맨드가 호출된다

**Then**
- `check_update()`는 다음을 반환해야 한다:
  ```json
  {
    "hasUpdate": false,
    "latestVersion": "0.3.0",
    "url": "https://github.com/owner/repo/releases/tag/v0.3.0"
  }
  ```
- 프론트엔드 UI는 "최신 버전입니다" 메시지를 표시한다
- 앱은 정상 동작을 계속한다 (크래시 없음)

**검증 방법**
- 단위 테스트: 동일 버전 비교 로직 검증
- `cargo test` 실행 후 모든 테스트 통과 확인

---

## Scenario 3: GitHub API 호출 실패 시 안전한 폴백

**관련 요구사항**: REQ-001 (State-Driven + Unwanted)

**Given**
- GitHub API 서버가 응답하지 않거나 (타임아웃), HTTP 5xx 오류를 반환한다
- 또는 네트워크 연결이 없는 상태이다

**When**
- `check_update()` Tauri 커맨드가 호출된다

**Then**
- `check_update()`는 다음을 반환해야 한다:
  ```json
  {
    "hasUpdate": false,
    "latestVersion": null,
    "url": null
  }
  ```
- 앱은 패닉/크래시 없이 정상 실행을 유지한다
- 오류는 로그에 기록된다 (eprintln! 또는 tauri log)
- 프론트엔드는 오류 상태를 표시하거나 "확인 실패" 메시지를 표시한다

**검증 방법**
- 단위 테스트: `reqwest` 오류 시뮬레이션
- `cargo test` 실행 후 모든 테스트 통과 확인
- 프론트엔드 통합 테스트 (선택적)

---

## Scenario 4: npm run release --patch로 패치 버전 증가 및 태그 푸시

**관련 요구사항**: REQ-002

**Given**
- 현재 `package.json` 버전이 `"0.2.5"`이다
- Git working tree가 clean하다 (`git status --porcelain` 출력이 비어있음)
- Git 원격 저장소(origin)에 쓰기 권한이 있다

**When**
- 개발자가 `npm run release -- --patch` 명령어를 실행한다

**Then**
- 스크립트는 새 버전 `0.2.6`을 계산해야 한다
- 다음 세 파일의 버전이 `0.2.6`으로 업데이트되어야 한다:
  - `package.json`: `"version": "0.2.6"`
  - `src-tauri/tauri.conf.json`: `"version": "0.2.6"`
  - `src-tauri/Cargo.toml`: `version = "0.2.6"`
- Git 커밋 `chore(release): v0.2.6`이 생성되어야 한다
- Git 태그 `v0.2.6`이 생성되어야 한다
- 커밋과 태그가 원격 저장소에 푸시되어야 한다
- 성공 메시지가 출력된다: `Released v0.2.6 — CI will build and publish artifacts`

**검증 방법**
- `git log --oneline -1` 출력에서 커밋 메시지 확인
- `git tag -l v0.2.6` 명령어로 태그 존재 확인
- `package.json`, `tauri.conf.json`, `Cargo.toml` 파일에서 버전 필드 확인
- GitHub Actions 트리거 확인 (CI 파이프라인 시작 여부)

---

## Scenario 5: Dirty Working Tree 시 릴리즈 거부

**관련 요구사항**: REQ-002 (Unwanted)

**Given**
- 현재 `package.json` 버전이 `"0.2.5"`이다
- Git working tree에 커밋되지 않은 변경사항이 존재한다 (예: `src/renderer/App.tsx` 수정됨)

**When**
- 개발자가 `npm run release -- --patch` 명령어를 실행한다

**Then**
- 스크립트는 즉시 오류를 출력해야 한다:
  ```
  Error: Working tree is dirty. Please commit or stash your changes before releasing.
  ```
- 스크립트는 exit code 1로 종료되어야 한다
- 어떠한 파일도 수정되지 않아야 한다 (version bump 없음)
- 어떠한 git 커밋이나 태그도 생성되지 않아야 한다

**검증 방법**
- 테스트 파일에 수정 사항을 만든 후 스크립트 실행
- `echo $?` 명령어로 exit code 1 확인
- `git log --oneline -1` 출력이 변경되지 않았는지 확인
- `package.json` 버전이 `0.2.5`로 유지되는지 확인

---

## Scenario 6: CI release.yml에서 Cargo.toml 버전 동기화

**관련 요구사항**: REQ-003

**Given**
- GitHub Actions release.yml이 git 태그 `v0.3.0` 푸시로 트리거된다
- 태그 이전 `src-tauri/Cargo.toml`의 `[package].version`이 `"0.1.0"` (또는 이전 버전)이다

**When**
- release.yml CI 파이프라인이 실행된다

**Then**
- CI 내에서 `src-tauri/Cargo.toml`의 `version = "..."` 라인이 `version = "0.3.0"`으로 업데이트되어야 한다
- `package.json`의 버전도 `0.3.0`으로 업데이트되어야 한다 (기존 동작)
- `src-tauri/tauri.conf.json`의 버전도 `0.3.0`으로 업데이트되어야 한다 (기존 동작)
- Tauri 빌드가 `0.3.0` 버전으로 성공적으로 완료된다
- GitHub Release `v0.3.0`이 생성된다

**검증 방법**
- GitHub Actions 로그에서 "Sync Cargo.toml version" 스텝 성공 확인
- 빌드된 바이너리에서 버전 정보 확인 (Tauri 앱 About 화면)
- 릴리즈 아티팩트 파일명에서 버전 일치 확인

---

## Quality Gate 기준

| 기준 | 임계값 | 측정 방법 |
|------|--------|----------|
| Rust 테스트 통과율 | 100% | `cargo test` |
| Rust 컴파일 경고 | 0개 | `cargo build --release` |
| Rust 컴파일 오류 | 0개 | `cargo build --release` |
| check_update() 응답 시간 | 10초 이내 | 타임아웃 설정으로 보장 |
| release.js 오류 처리 | 모든 edge case 처리 | 수동 테스트 |
| 세 파일 버전 일치 | 항상 일치 | 릴리즈 후 검증 |

---

## Definition of Done

다음 조건이 모두 충족될 때 SPEC-UPDATE-001이 완료된 것으로 간주한다:

- [ ] `check_update()`가 GitHub Releases API를 실제 호출하고 올바른 결과를 반환한다
- [ ] 네트워크 오류 시 앱이 크래시 없이 `hasUpdate: false`를 반환한다
- [ ] `npm run release -- --patch|--minor|--major|--version X.Y.Z`가 정상 동작한다
- [ ] dirty working tree 시 릴리즈 스크립트가 exit 1로 중단된다
- [ ] 릴리즈 후 세 파일(package.json, tauri.conf.json, Cargo.toml)의 버전이 동일하다
- [ ] CI release.yml에서 Cargo.toml 버전도 동기화된다
- [ ] `cargo test`가 100% 통과한다
- [ ] 기존 프론트엔드 인터페이스(desktopClient.ts)가 변경 없이 동작한다
- [ ] 코드 변경에 적절한 @MX:NOTE 또는 @MX:ANCHOR 태그가 추가된다

---

## TAG Block

```
SPEC-ID: SPEC-UPDATE-001
DOMAIN: UPDATE
ACCEPTANCE-VERSION: 0.1.0
SCENARIOS: 6
METHODOLOGY: ddd
REQ-COVERAGE: [REQ-001, REQ-002, REQ-003, REQ-004]
```
