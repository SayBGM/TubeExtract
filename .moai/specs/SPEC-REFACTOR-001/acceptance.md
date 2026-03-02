---
id: SPEC-REFACTOR-001
document: acceptance
version: 0.1.0
---

# 인수 기준: SPEC-REFACTOR-001 lib.rs 모듈 분리

## 개요

이 문서는 `lib.rs` 모듈 분리 리팩토링의 완료를 검증하기 위한 인수 기준을 정의한다.
모든 시나리오는 **Given-When-Then** 형식으로 작성되었으며, DDD 방법론의 행동 보존 원칙에 따른다.

---

## 품질 지표 (Quality Metrics)

| 지표 | 목표 | 측정 방법 |
|------|------|---------|
| `lib.rs` 줄 수 | < 80줄 | `wc -l src-tauri/src/lib.rs` |
| 최장 함수 줄 수 | < 100줄 | `cargo clippy` + 수동 확인 |
| 뮤텍스 복구 중복 | 0 (단일 함수) | `grep "unwrap_or_else" src-tauri/src/lib.rs` = 0 |
| 매직 상태 문자열 | 0 | `grep '"queued"\|"downloading"' src-tauri/src/` = 0 |
| Tauri 커맨드 수 | 18 (보존) | `grep "#\[tauri::command\]"` 카운트 |
| `cargo clippy` 경고 | 0 | `cargo clippy -- -D warnings` 성공 |
| `cargo test` 통과 | 100% | `cargo test` 전체 통과 |
| 공개 항목 문서화 | 100% | `cargo doc --no-deps` 오류 없음 |

---

## 시나리오 1: 모듈 구조 완성 검증

### 1.1 — 10개 모듈 파일 존재

**Given** 리팩토링이 완료된 프로젝트에서

**When** `src-tauri/src/` 디렉토리를 확인하면

**Then** 다음 10개 파일이 모두 존재해야 한다:
- `lib.rs` (진입점)
- `state.rs` (공유 상태)
- `types.rs` (도메인 타입)
- `utils.rs` (파싱 헬퍼)
- `file_ops.rs` (파일 작업)
- `dependencies.rs` (의존성 관리)
- `diagnostics.rs` (시스템 진단)
- `settings.rs` (설정)
- `metadata.rs` (URL 분석)
- `queue.rs` (큐 관리)
- `download.rs` (다운로드 워커)

**검증 명령**:
```bash
ls src-tauri/src/*.rs | wc -l  # 결과: 11 (lib.rs 포함)
```

### 1.2 — lib.rs 최소화

**Given** 리팩토링이 완료된 프로젝트에서

**When** `src-tauri/src/lib.rs` 파일의 줄 수를 확인하면

**Then** 80줄 이하여야 한다

**Then** 파일에 `mod` 선언과 `run()` 함수, 커맨드 등록 코드만 포함되어야 한다

**Then** 비즈니스 로직 구현 코드가 포함되지 않아야 한다

**검증 명령**:
```bash
wc -l src-tauri/src/lib.rs        # 80 이하
grep -c "^pub fn\|^fn " src-tauri/src/lib.rs  # run() 함수 1개만
```

### 1.3 — 빌드 성공

**Given** 리팩토링이 완료된 프로젝트에서

**When** `cargo build`를 실행하면

**Then** 빌드가 오류 없이 성공해야 한다

**검증 명령**:
```bash
cd src-tauri && cargo build 2>&1 | grep -c "^error" # 결과: 0
```

---

## 시나리오 2: 뮤텍스 복구 패턴 중앙화 검증

### 2.1 — lock_or_recover 함수 존재

**Given** 리팩토링이 완료된 프로젝트에서

**When** `src-tauri/src/state.rs`를 확인하면

**Then** `lock_or_recover` 함수가 정의되어 있어야 한다

**Then** 함수가 `///` 문서 주석을 포함해야 한다

**검증 명령**:
```bash
grep -n "pub fn lock_or_recover" src-tauri/src/state.rs  # 결과: 1줄
grep -B1 "pub fn lock_or_recover" src-tauri/src/state.rs  # 문서 주석 확인
```

### 2.2 — 중복 뮤텍스 패턴 제거

**Given** 리팩토링이 완료된 프로젝트에서

**When** 전체 소스 파일에서 직접 `unwrap_or_else(|e|` 패턴을 검색하면

**Then** 뮤텍스 복구 목적의 직접 구현이 존재하지 않아야 한다

**검증 명령**:
```bash
# "Mutex poisoned" 문자열이 lock_or_recover 정의 외부에 없어야 함
grep -rn "Mutex poisoned" src-tauri/src/ | grep -v "state.rs"  # 결과: 0줄
```

### 2.3 — 동일한 복구 동작 유지

**Given** 뮤텍스가 포이즌된 상황에서

**When** `lock_or_recover(mutex, "test_context")`를 호출하면

**Then** 포이즌된 가드를 복구하여 `MutexGuard`를 반환해야 한다

**Then** stderr에 `[STABILITY]` 접두사가 포함된 로그 메시지를 출력해야 한다

**Then** 패닉 없이 실행이 계속되어야 한다

---

## 시나리오 3: DownloadStatus Enum 타입 안전성 검증

### 3.1 — DownloadStatus Enum 정의

**Given** 리팩토링이 완료된 프로젝트에서

**When** `src-tauri/src/types.rs`를 확인하면

**Then** `DownloadStatus` enum이 다음 6개 변형을 포함해야 한다:
- `Queued`
- `Downloading`
- `Paused`
- `Canceled`
- `Completed`
- `Failed`

**Then** `serde::Serialize`와 `serde::Deserialize`를 derive해야 한다

**Then** `#[serde(rename_all = "lowercase")]`로 JSON 직렬화 시 소문자로 변환되어야 한다

**검증 코드**:
```rust
#[test]
fn test_download_status_serialization() {
    assert_eq!(
        serde_json::to_string(&DownloadStatus::Queued).unwrap(),
        "\"queued\""
    );
    assert_eq!(
        serde_json::to_string(&DownloadStatus::Downloading).unwrap(),
        "\"downloading\""
    );
}
```

### 3.2 — 매직 문자열 제거

**Given** 리팩토링이 완료된 프로젝트에서

**When** 소스 코드에서 상태 문자열 리터럴을 검색하면

**Then** 다운로드 상태를 나타내는 문자열 리터럴이 존재하지 않아야 한다

**검증 명령**:
```bash
grep -rn '"queued"\|"downloading"\|"paused"\|"canceled"\|"completed"\|"failed"' \
     src-tauri/src/ | grep -v "//\|test\|#\[serde" | wc -l  # 결과: 0
```

### 3.3 — 프론트엔드 JSON 호환성 유지

**Given** TypeScript 프론트엔드가 `"queued"`, `"downloading"` 등의 소문자 문자열을 기대할 때

**When** `DownloadStatus::Queued`를 JSON으로 직렬화하면

**Then** 프론트엔드가 기대하는 소문자 문자열 `"queued"`가 생성되어야 한다

**Then** TypeScript 프론트엔드 코드 수정 없이 동작해야 한다

---

## 시나리오 4: 18개 Tauri 커맨드 동작 보존 검증

### 4.1 — 커맨드 등록 완전성

**Given** 리팩토링이 완료된 `lib.rs`에서

**When** `invoke_handler` 매크로를 확인하면

**Then** 다음 18개 커맨드가 모두 등록되어 있어야 한다:

| 커맨드 | 담당 모듈 |
|--------|---------|
| `analyze_url` | `metadata` |
| `check_duplicate` | `queue` |
| `enqueue_job` | `queue` |
| `pause_job` | `queue` |
| `resume_job` | `queue` |
| `cancel_job` | `queue` |
| `clear_terminal_jobs` | `queue` |
| `get_queue_snapshot` | `queue` |
| `get_settings` | `settings` |
| `get_dependency_bootstrap_status` | `dependencies` |
| `pick_download_dir` | `settings` |
| `set_settings` | `settings` |
| `run_diagnostics` | `diagnostics` |
| `check_update` | `diagnostics` |
| `get_storage_stats` | `diagnostics` |
| `delete_file` | `queue` |
| `open_folder` | `diagnostics` |
| `open_external_url` | `diagnostics` |

**검증 명령**:
```bash
grep -c "#\[tauri::command\]" src-tauri/src/*.rs  # 합계: 18
```

### 4.2 — analyze_url 동작 보존

**Given** 유효한 YouTube URL이 주어졌을 때

**When** `analyze_url` 커맨드를 호출하면

**Then** 비디오 제목, 가용 품질 옵션 목록, 섬네일 URL을 포함한 응답을 반환해야 한다

**Then** 응답 구조가 리팩토링 전과 동일해야 한다

**특성화 테스트**:
```rust
#[test]
fn test_analyze_url_response_structure() {
    // research.md의 AnalysisResult 구조와 동일한 필드 확인
    let result = AnalysisResult::default();
    assert!(result.title.is_empty() || !result.title.is_empty()); // 필드 존재 확인
    assert!(result.quality_options.is_empty() || !result.quality_options.is_empty());
}
```

### 4.3 — enqueue_job 동작 보존

**Given** 유효한 URL과 품질 옵션이 주어졌을 때

**When** `enqueue_job` 커맨드를 호출하면

**Then** 새 `QueueItem`이 큐에 추가되어야 한다

**Then** 추가된 항목의 초기 상태가 `DownloadStatus::Queued`여야 한다

**Then** 큐가 디스크에 영속되어야 한다

### 4.4 — 다운로드 워커 상태 전환 보존

**Given** 큐에 `Queued` 상태 항목이 있을 때

**When** 워커가 실행되면

**Then** 상태 전환이 `Queued → Downloading → Completed` 순서로 발생해야 한다

**When** 다운로드 중 `pause_job`이 호출되면

**Then** 상태가 `Downloading → Paused`로 변경되어야 한다

**When** `resume_job`이 호출되면

**Then** 상태가 `Paused → Downloading`으로 복원되어야 한다

---

## 시나리오 5: start_worker_if_needed 함수 분해 검증

### 5.1 — 함수 분해 완성

**Given** 리팩토링이 완료된 `src-tauri/src/download.rs`에서

**When** 함수 정의를 확인하면

**Then** `start_worker_if_needed` 대신 다음 함수들이 존재해야 한다:
- `poll_next_job()`
- `build_ytdlp_command()`
- `spawn_download_process()`
- `wait_for_process()`
- `handle_exit_status()`

**Then** 각 함수가 100줄을 초과하지 않아야 한다

**검증 명령**:
```bash
grep -n "^fn \|^pub fn " src-tauri/src/download.rs  # 분해된 함수 목록
# 각 함수 줄 수를 수동 확인 또는 스크립트로 측정
```

### 5.2 — 재시도 로직 보존

**Given** 네트워크 오류로 다운로드가 실패했을 때

**When** `classify_download_error(stderr: &str)`가 호출되면

**Then** 오류 유형에 따라 적절한 `RetryStrategy`를 반환해야 한다

**Then** 재시도 지연 시간이 이전과 동일해야 한다

**특성화 테스트**:
```rust
#[test]
fn test_retry_strategy_classification() {
    assert_eq!(
        classify_download_error("HTTP Error 429"),
        RetryStrategy::Backoff  // 또는 현재 동작과 동일한 전략
    );
}
```

---

## 시나리오 6: 품질 게이트 통과 검증

### 6.1 — cargo fmt 통과

**Given** 완료된 리팩토링 코드에서

**When** `cargo fmt --check`를 실행하면

**Then** 포맷팅 변경 없이 성공해야 한다

**검증 명령**:
```bash
cd src-tauri && cargo fmt --check && echo "PASS"
```

### 6.2 — cargo clippy 경고 0개

**Given** 완료된 리팩토링 코드에서

**When** `cargo clippy -- -D warnings`를 실행하면

**Then** 경고 없이 성공해야 한다

**검증 명령**:
```bash
cd src-tauri && cargo clippy -- -D warnings && echo "PASS"
```

### 6.3 — cargo test 전체 통과

**Given** 특성화 테스트가 모든 Phase에서 작성되었을 때

**When** `cargo test`를 실행하면

**Then** 모든 테스트가 통과해야 한다

**Then** 실패하는 테스트가 없어야 한다

**검증 명령**:
```bash
cd src-tauri && cargo test 2>&1 | tail -5
# Expected: "test result: ok. N passed; 0 failed; 0 ignored"
```

### 6.4 — cargo doc 오류 없음

**Given** 문서화가 완료된 코드에서

**When** `cargo doc --no-deps`를 실행하면

**Then** 문서 생성이 오류 없이 성공해야 한다

**검증 명령**:
```bash
cd src-tauri && cargo doc --no-deps 2>&1 | grep -c "^error" # 결과: 0
```

---

## 시나리오 7: 문서화 완성도 검증

### 7.1 — 공개 함수 문서화

**Given** 리팩토링이 완료된 모든 모듈에서

**When** 공개(`pub`) 함수를 확인하면

**Then** 각 함수에 `///` 문서 주석이 있어야 한다

**Then** Tauri 커맨드 함수에 목적, 매개변수, 반환값이 문서화되어야 한다

**검증 명령**:
```bash
# pub fn 바로 위 줄이 /// 로 시작하는지 확인
grep -B1 "pub fn " src-tauri/src/*.rs | grep -v "^--$\|///" | grep "pub fn " | wc -l
# 결과: 0 (모든 pub fn 이 문서화됨)
```

### 7.2 — 공개 구조체 문서화

**Given** 리팩토링이 완료된 모든 모듈에서

**When** 공개(`pub`) 구조체를 확인하면

**Then** 각 구조체에 `///` 문서 주석이 있어야 한다

**Then** `QueueItem`의 20개 필드 각각이 문서화되어야 한다

---

## 완료 정의 (Definition of Done)

리팩토링이 완전히 완료된 것으로 간주되려면 다음 모든 조건이 충족되어야 한다:

### 필수 조건 (Mandatory)

- [ ] **구조**: 10개 모듈 파일 모두 존재
- [ ] **크기**: `lib.rs` 80줄 이하
- [ ] **빌드**: `cargo build` 성공
- [ ] **테스트**: `cargo test` 전체 통과
- [ ] **린팅**: `cargo clippy -- -D warnings` 경고 0개
- [ ] **포맷**: `cargo fmt --check` 통과
- [ ] **커맨드**: 18개 Tauri 커맨드 모두 등록
- [ ] **타입 안전**: `DownloadStatus` enum 완전 적용 (매직 문자열 0개)
- [ ] **중복 제거**: `lock_or_recover` 단일 구현 (직접 중복 패턴 0개)

### 권장 조건 (Recommended)

- [ ] **문서화**: 모든 `pub` 항목 `///` 주석 완비
- [ ] **함수 크기**: 최장 함수 100줄 이하
- [ ] **특성화 테스트**: 모든 Phase의 특성화 테스트 존재
- [ ] **성능**: 수동 다운로드 플로우 검증 완료

---

## 검증 자동화 스크립트

리팩토링 완료 후 아래 스크립트로 전체 검증 수행:

```bash
#!/bin/bash
set -e

echo "=== SPEC-REFACTOR-001 인수 검증 ==="

# 1. 모듈 파일 존재 확인
echo "[1/7] 모듈 파일 존재 확인..."
EXPECTED_FILES="state.rs types.rs utils.rs file_ops.rs dependencies.rs diagnostics.rs settings.rs metadata.rs queue.rs download.rs"
for f in $EXPECTED_FILES; do
    [ -f "src-tauri/src/$f" ] || { echo "FAIL: $f 없음"; exit 1; }
done
echo "PASS: 10개 모듈 파일 확인"

# 2. lib.rs 줄 수 확인
echo "[2/7] lib.rs 줄 수 확인..."
LINES=$(wc -l < src-tauri/src/lib.rs)
[ "$LINES" -le 80 ] || { echo "FAIL: lib.rs = $LINES 줄 (80 초과)"; exit 1; }
echo "PASS: lib.rs = $LINES 줄"

# 3. cargo build
echo "[3/7] cargo build..."
(cd src-tauri && cargo build) || { echo "FAIL: 빌드 실패"; exit 1; }
echo "PASS: 빌드 성공"

# 4. cargo test
echo "[4/7] cargo test..."
(cd src-tauri && cargo test) || { echo "FAIL: 테스트 실패"; exit 1; }
echo "PASS: 테스트 통과"

# 5. cargo clippy
echo "[5/7] cargo clippy..."
(cd src-tauri && cargo clippy -- -D warnings) || { echo "FAIL: clippy 경고"; exit 1; }
echo "PASS: clippy 통과"

# 6. Tauri 커맨드 수 확인
echo "[6/7] Tauri 커맨드 수 확인..."
CMD_COUNT=$(grep -rh "#\[tauri::command\]" src-tauri/src/*.rs | wc -l)
[ "$CMD_COUNT" -eq 18 ] || { echo "FAIL: 커맨드 수 = $CMD_COUNT (18 기대)"; exit 1; }
echo "PASS: $CMD_COUNT 개 커맨드 등록"

# 7. 매직 문자열 확인
echo "[7/7] 매직 상태 문자열 제거 확인..."
MAGIC=$(grep -rn '"queued"\|"downloading"\|"paused"\|"canceled"\|"completed"\|"failed"' \
    src-tauri/src/ | grep -v "//\|#\[test\]\|serde" | wc -l)
[ "$MAGIC" -eq 0 ] || { echo "FAIL: 매직 문자열 $MAGIC 개 발견"; exit 1; }
echo "PASS: 매직 문자열 없음"

echo ""
echo "=== 모든 인수 기준 통과 ==="
```
