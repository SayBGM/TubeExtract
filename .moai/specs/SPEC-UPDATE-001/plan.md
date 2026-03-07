---
spec-id: SPEC-UPDATE-001
type: plan
created: 2026-03-02
updated: 2026-03-02
methodology: ddd
---

# Plan: SPEC-UPDATE-001 — 자동 업데이트 및 릴리즈 시스템 구현

## 개요

DDD(ANALYZE-PRESERVE-IMPROVE) 방법론에 따라 총 5개의 페이즈로 구현한다. 기존 스텁 동작을 먼저 특성화 테스트로 문서화한 후 실제 구현으로 교체하는 전략을 사용한다.

---

## Phase 1: DDD ANALYZE — 현재 코드 분석

**목표**: 변경 대상 코드의 현재 구조와 동작을 완전히 이해한다.

### 분석 대상 파일

1. **`src-tauri/src/diagnostics.rs`** (lines 224-232)
   - `check_update()` 스텁 구조 확인
   - 반환 타입 `CommandResult<Value>` 구조 파악
   - `serde_json::json!` 매크로 사용 패턴 확인

2. **`src-tauri/Cargo.toml`**
   - 현재 의존성 목록 확인 (`reqwest` 부재 확인)
   - `[package].version` 필드 위치 확인
   - 기존 `features` 목록 확인

3. **`src-tauri/src/lib.rs`** (line 102)
   - `check_update` 커맨드 등록 방식 확인
   - 다른 진단 커맨드 등록 패턴 참조

4. **`.github/workflows/release.yml`** (lines 37-51)
   - 현재 버전 동기화 로직 구조 파악
   - `VERSION` 변수 추출 패턴 확인
   - Cargo.toml 동기화 누락 지점 식별

5. **`src/renderer/lib/desktopClient.ts`** (lines 294-301)
   - 프론트엔드 `checkUpdate()` 호출 인터페이스 확인
   - 반환값 타입 정의 확인

6. **`package.json`**
   - 현재 `scripts` 섹션 확인
   - 현재 버전 필드 확인

### 분석 완료 기준

- [ ] diagnostics.rs `check_update()` 현재 구현 완전 이해
- [ ] Cargo.toml 의존성 추가 위치 결정
- [ ] release.yml 버전 동기화 스텝 추가 위치 결정
- [ ] 프론트엔드 인터페이스 계약 문서화

---

## Phase 2: DDD PRESERVE — 특성화 테스트 작성

**목표**: 현재 `check_update()` 동작을 특성화 테스트로 캡처하여 회귀를 방지한다.

### 특성화 테스트 대상

**파일**: `src-tauri/src/diagnostics.rs` (테스트 모듈 추가)

**테스트 케이스**:

```rust
#[cfg(test)]
mod tests {
    // 특성화 테스트 1: 현재 스텁이 항상 hasUpdate: false 반환
    // (구현 교체 후 동작 변경 확인용 참조 테스트)

    // 특성화 테스트 2: 반환 구조체 형식 검증
    // hasUpdate (bool), latestVersion (nullable), url (nullable)
}
```

### 프론트엔드 인터페이스 불변식 문서화

**파일**: `src/renderer/lib/desktopClient.ts`

현재 인터페이스:
```typescript
interface CheckUpdateResult {
  hasUpdate: boolean;
  latestVersion: string | null;
  url: string | null;
}
```

이 인터페이스는 구현 교체 후에도 유지되어야 한다.

### PRESERVE 완료 기준

- [ ] 특성화 테스트 작성 완료
- [ ] 테스트 실행 (`cargo test`) 통과 확인
- [ ] 프론트엔드 인터페이스 계약 문서화 완료

---

## Phase 3: DDD IMPROVE — check_update() GitHub API 연동 (REQ-001)

**목표**: 스텁을 실제 GitHub Releases API 연동으로 교체한다.

### 태스크 분해

#### Task 3.1: Cargo.toml 의존성 추가

**파일**: `src-tauri/Cargo.toml`

추가할 의존성:
```toml
reqwest = { version = "0.12", features = ["json", "rustls-tls"], default-features = false }
```

#### Task 3.2: diagnostics.rs 구현 교체

**파일**: `src-tauri/src/diagnostics.rs`

구현 전략:
1. GitHub owner/repo를 상수로 정의 (또는 tauri.conf.json에서 읽기)
2. `reqwest::Client` 생성 (타임아웃 10초)
3. GitHub API 호출: `GET /repos/{owner}/{repo}/releases/latest`
4. `User-Agent: TubeExtract/{version}` 헤더 필수
5. 응답에서 `tag_name`과 `html_url` 추출
6. `tag_name`에서 버전 파싱 (앞의 `v` 제거)
7. `tauri::app::App::package_info().version`과 비교
8. 비교 결과로 `hasUpdate`, `latestVersion`, `url` 반환

#### Task 3.3: 통합 테스트 작성

**접근 방법**: mock 서버 또는 조건부 컴파일을 사용한 단위 테스트

**테스트 케이스**:
- 최신 버전이 있을 때 `hasUpdate: true` 반환 확인
- 최신 버전이 없을 때 `hasUpdate: false` 반환 확인
- API 실패 시 `hasUpdate: false` 반환 확인 (패닉 없음)

### Phase 3 완료 기준

- [ ] Cargo.toml에 reqwest 추가
- [ ] `check_update()` 실제 구현 완료
- [ ] 특성화 테스트 여전히 통과 (인터페이스 유지 확인)
- [ ] 새 통합 테스트 통과
- [ ] `cargo build` 경고/오류 없음

---

## Phase 4: DDD IMPROVE — scripts/release.js 생성 (REQ-002)

**목표**: 개발자용 릴리즈 자동화 스크립트를 구현한다.

### 태스크 분해

#### Task 4.1: scripts/ 디렉토리 생성 및 release.js 작성

**파일**: `scripts/release.js` (신규)

**구현 단계별 세부사항**:

```
Step 1: 인자 파싱
  - process.argv에서 --patch, --minor, --major, --version 추출
  - 인자 없으면 사용법 출력 후 exit(1)

Step 2: Git Working Tree 검증
  - `git status --porcelain` 실행
  - 출력이 비어있지 않으면 "Working tree is dirty" 오류 후 exit(1)

Step 3: 현재 버전 읽기
  - package.json 읽기 → JSON.parse → .version 추출

Step 4: 새 버전 계산
  - --patch: PATCH + 1
  - --minor: MINOR + 1, PATCH = 0
  - --major: MAJOR + 1, MINOR = 0, PATCH = 0
  - --version X.Y.Z: 검증 후 사용

Step 5: 파일 버전 업데이트
  - package.json: JSON 파싱 → version 필드 수정 → 쓰기
  - src-tauri/tauri.conf.json: JSON 파싱 → version 필드 수정 → 쓰기
  - src-tauri/Cargo.toml: 정규식으로 version = "..." 라인 교체

Step 6: Git 작업
  - git add package.json src-tauri/tauri.conf.json src-tauri/Cargo.toml
  - git commit -m "chore(release): v{version}"
  - git tag v{version}
  - git push
  - git push --tags

Step 7: 성공 출력
  - "Released v{version} — CI will build and publish artifacts"
```

#### Task 4.2: package.json 스크립트 등록

**파일**: `package.json`

```json
{
  "scripts": {
    "release": "node scripts/release.js"
  }
}
```

### Phase 4 완료 기준

- [ ] `scripts/release.js` 생성 완료
- [ ] `package.json`에 `release` 스크립트 등록
- [ ] dirty working tree 감지 동작 확인 (수동 테스트)
- [ ] semver bump 계산 정확성 확인
- [ ] 세 파일 버전 업데이트 동작 확인

---

## Phase 5: DDD IMPROVE — release.yml Cargo.toml 버전 동기화 (REQ-003)

**목표**: GitHub Actions 릴리즈 파이프라인에서 Cargo.toml 버전도 동기화하도록 수정한다.

### 태스크 분해

#### Task 5.1: release.yml 수정

**파일**: `.github/workflows/release.yml`

**삽입 위치**: 기존 tauri.conf.json 버전 업데이트 스텝 직후

**추가할 스텝**:
```yaml
- name: Sync Cargo.toml version
  run: |
    sed -i "s/^version = \".*\"/version = \"$VERSION\"/" src-tauri/Cargo.toml
```

**플랫폼 호환성 주의**:
- macOS의 `sed -i`는 빈 백업 확장자 필요: `sed -i '' "s/.../.../"`
- Linux/Ubuntu는 `sed -i "s/.../.../"` 사용
- 빌드 매트릭스가 macOS와 Windows를 모두 포함하므로 OS별 분기 필요

#### Task 5.2: 동기화 검증

- [ ] release.yml에 Cargo.toml 동기화 스텝 추가
- [ ] macOS와 Linux 플랫폼 모두에서 sed 명령 호환성 확인
- [ ] 변경 후 테스트 태그로 CI 파이프라인 검증

### Phase 5 완료 기준

- [ ] release.yml Cargo.toml 동기화 스텝 추가
- [ ] 플랫폼별 sed 호환성 처리
- [ ] CI 실행 결과에서 세 파일 버전 일치 확인

---

## 의존성 및 순서

```
Phase 1 (ANALYZE)
    │
    ▼
Phase 2 (PRESERVE) ──── Phase 1 완료 필요
    │
    ▼
Phase 3 (check_update 구현) ──── Phase 2 완료 필요
    │
Phase 4 (release.js) ──── Phase 1 완료 필요 (Phase 2, 3과 병렬 가능)
    │
Phase 5 (release.yml) ──── Phase 1 완료 필요 (Phase 2, 3, 4와 병렬 가능)
```

Phase 3, 4, 5는 Phase 2 이후 병렬 실행 가능하나, 단일 개발자 환경에서는 순차 실행 권장.

---

## 리스크 및 대응 방안

| 리스크 | 발생 가능성 | 영향도 | 대응 방안 |
|-------|-----------|--------|----------|
| GitHub API rate limit (60 req/hour for unauthenticated) | 낮음 | 낮음 | 캐싱 고려 또는 에러 메시지 안내 |
| reqwest + rustls-tls 컴파일 시간 증가 | 중간 | 낮음 | 기존 의존성 재사용 가능 여부 먼저 확인 |
| macOS/Linux sed 명령 차이 | 높음 | 중간 | OS 감지 후 분기 또는 Python 사용 |
| Cargo.toml 정규식이 workspace 설정과 충돌 | 낮음 | 높음 | `[package]` 섹션 내 버전만 대상 지정 |
| git push 권한 부족 (scripts/release.js) | 낮음 | 높음 | 사전 SSH 키 설정 가이드 문서화 |

---

## 우선순위

| 우선순위 | 페이즈 | 설명 |
|---------|--------|------|
| Primary Goal | Phase 1, 2, 3 | check_update() 실제 동작 구현 (핵심 기능) |
| Secondary Goal | Phase 4 | 릴리즈 자동화 스크립트 (개발 편의성) |
| Final Goal | Phase 5 | CI Cargo.toml 버전 동기화 (일관성 보장) |

---

## TAG Block

```
SPEC-ID: SPEC-UPDATE-001
DOMAIN: UPDATE
PLAN-VERSION: 0.1.0
PHASES: [ANALYZE, PRESERVE, IMPROVE-x3]
METHODOLOGY: ddd
```
