---
id: SPEC-REFACTOR-001
document: plan
version: 0.1.0
---

# 구현 계획: SPEC-REFACTOR-001 lib.rs 모듈 분리

## 개요

이 계획은 DDD(Domain-Driven Development) 방법론의 **ANALYZE-PRESERVE-IMPROVE** 사이클에 따라
`src-tauri/src/lib.rs` (2,491줄)를 10개 모듈로 점진적으로 분리한다.

각 Phase 완료 후 반드시 품질 게이트(`cargo build`, `cargo test`, `cargo clippy`)를
통과해야 다음 Phase로 진행한다.

---

## 전제 조건 (ANALYZE 단계 완료)

research.md에 의해 분석이 완료되었다:

- [x] 2,491줄 파일 전체 구조 파악
- [x] 18개 Tauri 커맨드 목록 및 도메인 분류
- [x] 6개 도메인 경계 식별
- [x] 순환 의존성 없음 확인
- [x] 핵심 위험 요소 식별 (311줄 워커, 8x 중복 패턴, 20+ 매직 문자열)

---

## Phase 1: Foundation — 기반 타입 추출 (최저 위험)

### 목표

의존성이 없는 순수 타입과 헬퍼를 먼저 추출하여 이후 Phase의 기반을 마련한다.

### 특성화 테스트 작성 (PRESERVE)

Phase 1 시작 전 작성해야 할 특성화 테스트:

```
tests/characterization/
├── test_download_status_strings.rs  # 현재 상태 문자열 값 캡처
├── test_mutex_recovery.rs            # 뮤텍스 복구 동작 캡처
└── test_pure_utils.rs                # 파싱 헬퍼 현재 출력 캡처
```

### 추출 순서

**단계 1.1: `state.rs` 생성**

추출 대상:
- `AppState` 구조체 (lib.rs 193-197줄)
- `SharedState` 타입 별칭 (lib.rs 200-201줄)
- `SharedRuntime` 타입 별칭 (lib.rs 220-221줄)
- `DependencyRuntimeState` 구조체 (lib.rs 226-230줄)

신규 추가:
- `lock_or_recover<T>()` 헬퍼 함수 구현

```rust
/// 뮤텍스 잠금 획득. 포이즌된 경우 복구하여 반환.
///
/// # Arguments
/// * `mutex` - 잠글 Arc<Mutex<T>>
/// * `context` - 로그 메시지에 포함할 컨텍스트 설명
pub fn lock_or_recover<'a, T>(
    mutex: &'a Arc<Mutex<T>>,
    context: &str,
) -> MutexGuard<'a, T> {
    mutex.lock().unwrap_or_else(|e| {
        eprintln!("[STABILITY] Mutex poisoned in {context}, recovering: {e:?}");
        e.into_inner()
    })
}
```

검증: `lib.rs`에서 8개 직접 패턴을 `lock_or_recover()` 호출로 교체 후 `cargo build` 성공.

**단계 1.2: `types.rs` 생성**

추출 대상:
- `DownloadStatus` enum 신규 정의
- `CommandResult<T>` 타입 별칭

`DownloadStatus` 직렬화 전략:
```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "lowercase")]
pub enum DownloadStatus {
    Queued,
    Downloading,
    Paused,
    Canceled,
    Completed,
    Failed,
}

impl std::fmt::Display for DownloadStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // serde lowercase 직렬화와 일치
        write!(f, "{}", serde_json::to_string(self).unwrap().trim_matches('"'))
    }
}
```

작업: `lib.rs`에서 문자열 비교 20+개를 `DownloadStatus` enum 비교로 교체
검증: `cargo build` 성공 + 특성화 테스트 통과

**단계 1.3: `utils.rs` 생성**

추출 대상 함수들 (lib.rs 1265-1384줄 주변):
- `parse_progress_percent(line: &str) -> Option<f64>`
- `parse_speed(line: &str) -> Option<String>`
- `parse_eta(line: &str) -> Option<String>`
- `append_download_log(job_id: &str, line: &str)`
- `select_format_expression(mode: &DownloadMode) -> String`
- `expected_extension(mode: &DownloadMode) -> &str`
- `sanitize_file_name(name: &str) -> String`
- `normalize_youtube_video_url(url: &str) -> String`

검증: 단위 테스트 포함하여 `cargo test` 통과

### Phase 1 품질 게이트

- [ ] `cargo fmt` — 포맷팅 통과
- [ ] `cargo clippy -- -D warnings` — 경고 0개
- [ ] `cargo build` — 빌드 성공
- [ ] `cargo test` — 특성화 테스트 포함 전체 통과
- [ ] `lib.rs` 줄 수 감소 확인 (2,491 → ~2,200줄 예상)

---

## Phase 2: Utilities — 유틸리티 모듈 추출 (낮은 위험)

### 목표

Phase 1 기반 위에 파일 작업, 의존성 관리, 진단 모듈을 추출한다.

### 특성화 테스트 작성 (PRESERVE)

```
tests/characterization/
├── test_file_ops_atomic.rs      # 원자적 쓰기/이동 동작 캡처
├── test_dependency_bootstrap.rs # 의존성 상태 확인 로직 캡처
└── test_diagnostics_output.rs   # 진단 결과 구조 캡처
```

### 추출 순서

**단계 2.1: `file_ops.rs` 생성**

추출 대상:
- `write_atomic()` (lib.rs 878-886줄)
- `move_file_atomic()` (lib.rs 1130-1187줄)
- `resolve_downloaded_file_path()` (lib.rs 1189-1222줄)
- `resolve_executable()` (lib.rs 356-383줄)
- `run_command_capture()` (lib.rs 413-539줄, 126줄 → 분해)
- 경로 헬퍼 함수들: `app_data_dir()`, `temp_downloads_root_dir()`, `queue_file_path()`,
  `settings_file_path()`, `settings_backup_file_path()`, `queue_backup_file_path()`

`run_command_capture` 분해:
```
run_command_capture() (~30줄: 조합 함수)
├── build_command() (~15줄: Command 구성)
└── run_with_watchdog() (~40줄: 타임아웃 워치독)
```

**단계 2.2: `dependencies.rs` 생성**

추출 대상:
- `bootstrap_dependencies()` (lib.rs 791-801줄)
- `ensure_ytdlp()` (lib.rs 617-682줄)
- `ensure_ffmpeg_available()` (lib.rs 684-711줄)
- `install_ffmpeg_windows()` (lib.rs 714-789줄) — `#[cfg(target_os = "windows")]` 보존
- `latest_ytdlp_version()` (lib.rs 564-584줄)
- `download_file()` (lib.rs 586-615줄)
- `wait_for_dependencies()` (lib.rs 833-855줄)

주의: 플랫폼 조건부 컴파일 블록 정확히 보존

**단계 2.3: `diagnostics.rs` 생성**

추출 대상:
- `run_diagnostics()` (lib.rs 2234-2293줄) — Tauri 커맨드
- `get_storage_stats()` (lib.rs 2343-2366줄) — Tauri 커맨드
- `open_folder()` (lib.rs 2381-2439줄) — Tauri 커맨드
- `open_external_url()` (lib.rs 2442-2475줄) — Tauri 커맨드
- `check_update()` (lib.rs 2334줄) — Tauri 커맨드 (stub)
- `can_write_to_dir()` (lib.rs 2295-2307줄)
- `calculate_directory_size()` (lib.rs 2313-2331줄)

개선: `open_folder`와 `open_external_url`의 플랫폼 분기 중복 제거:
```rust
fn open_with_platform_command(target: &str) -> Result<(), String>
```

### Phase 2 품질 게이트

- [ ] `cargo fmt` — 포맷팅 통과
- [ ] `cargo clippy -- -D warnings` — 경고 0개
- [ ] `cargo build` — 빌드 성공
- [ ] `cargo test` — 특성화 테스트 포함 전체 통과
- [ ] Tauri 커맨드 5개 추출 완료 (`run_diagnostics`, `get_storage_stats`, `open_folder`, `open_external_url`, `check_update`)

---

## Phase 3: Domain Logic — 도메인 로직 추출 (중간 위험)

### 목표

설정, 메타데이터, 큐 관리 도메인을 추출한다. 이 단계에서 Tauri 커맨드 다수가 이동된다.

### 특성화 테스트 작성 (PRESERVE)

```
tests/characterization/
├── test_settings_recovery.rs   # 3단계 폴백 복구 동작 캡처
├── test_queue_operations.rs    # 큐 추가/제거/상태변경 동작 캡처
└── test_metadata_parsing.rs    # URL 분석 결과 구조 캡처
```

### 추출 순서

**단계 3.1: `settings.rs` 생성**

추출 대상:
- `AppSettings` 구조체 (lib.rs 120-125줄)
- `PersistedSettings` 구조체 (lib.rs 184-190줄)
- `load_settings_with_recovery()` (lib.rs 932-984줄)
- `load_queue_with_recovery()` (lib.rs 1000-1058줄)
- `persist_settings()` (lib.rs 903-912줄)
- `normalize_download_dir()` (lib.rs 857-873줄)

개선: 두 복구 함수의 공통 3단계 폴백 로직 추출:
```rust
fn load_with_recovery<T: Default + DeserializeOwned>(
    primary: &Path,
    backup: &Path,
    context: &str,
) -> T
```

**단계 3.2: `metadata.rs` 생성**

추출 대상:
- `QualityOption` 구조체 (lib.rs 64-70줄)
- `AnalysisResult` 구조체 (lib.rs 73-86줄)
- `DownloadMode` enum (lib.rs 58-61줄)
- `analyze_url()` (lib.rs 1866-2033줄) — Tauri 커맨드 (165줄 → 분해)
- `normalize_youtube_video_url()` (utils.rs에서 재노출 또는 이동)

`analyze_url` 분해:
```
analyze_url() (~30줄: 조합 함수)
├── fetch_metadata_json() (~40줄: yt-dlp 실행 및 JSON 파싱)
├── parse_quality_options() (~60줄: 포맷 배열 파싱)
└── build_analysis_result() (~30줄: 최종 결과 구조 조합)
```

**단계 3.3: `queue.rs` 생성**

추출 대상:
- `QueueItem` 구조체 (lib.rs 90-111줄) — `status: DownloadStatus` 타입 사용
- `enqueue_job()` (lib.rs 2056-2102줄) — Tauri 커맨드
- `check_duplicate()` (lib.rs 2036-2055줄) — Tauri 커맨드
- `cancel_job()`, `pause_job()`, `resume_job()` — Tauri 커맨드
- `clear_terminal_jobs()` (lib.rs 2180-2191줄) — Tauri 커맨드
- `delete_file()` (lib.rs 2369-2380줄) — Tauri 커맨드
- `get_queue_snapshot()` (lib.rs 2192-2197줄) — Tauri 커맨드
- `persist_queue()` (lib.rs 891-899줄) — fan_in=8 유지
- `scan_incomplete_markers()` (lib.rs 1227-1263줄)
- `build_unique_output_path()` (lib.rs 1321-1349줄)
- `queue_snapshot()` (lib.rs 244-247줄)

주의: `queue.rs`는 `download.rs`의 워커 트리거를 호출하므로
`download.rs` 추출 후 참조를 업데이트해야 한다.

### Phase 3 품질 게이트

- [ ] `cargo fmt` — 포맷팅 통과
- [ ] `cargo clippy -- -D warnings` — 경고 0개
- [ ] `cargo build` — 빌드 성공
- [ ] `cargo test` — 특성화 테스트 포함 전체 통과
- [ ] 추출된 Tauri 커맨드: `analyze_url`, `enqueue_job`, `check_duplicate`, `pause_job`, `resume_job`, `cancel_job`, `clear_terminal_jobs`, `delete_file`, `get_queue_snapshot`, `set_settings`, `get_settings`, `pick_download_dir`

---

## Phase 4: Core Worker — 핵심 워커 추출 (높은 위험)

### 목표

가장 복잡한 `start_worker_if_needed()`를 포함한 `download.rs`를 추출한다.
이 Phase는 마지막에 수행해야 한다.

### 특성화 테스트 작성 (PRESERVE)

```
tests/characterization/
├── test_worker_state_transitions.rs  # 워커 상태 전환 동작 캡처
├── test_retry_classification.rs      # 오류 분류 → RetryStrategy 매핑 캡처
└── test_process_output_parsing.rs    # stdout/stderr 파싱 동작 캡처
```

### 추출 순서

**단계 4.1: `download.rs` 생성**

추출 대상:
- `RetryStrategy` enum (lib.rs 1070-1075줄)
- `ActiveProcess` 구조체 (lib.rs 203-207줄)
- `start_worker_if_needed()` (lib.rs 1552-1863줄) — **311줄 → 5개 함수로 분해**
- `classify_download_error()` (lib.rs 1077-1109줄)
- `retry_delay_ms_for_strategy()` (lib.rs 1111-1119줄)
- `handle_download_output_line()` (lib.rs 1399-1463줄)
- `kill_active_child_unchecked()` (lib.rs 1465-1483줄)
- `terminate_child_with_grace_period()` (lib.rs 1509-1544줄)

`start_worker_if_needed` 분해 계획:
```
start_worker_if_needed() (~50줄: 워커 루프 조합 함수)
├── poll_next_job() (~20줄)
│   └── 큐에서 'queued' 상태 항목 하나 꺼내기
├── build_ytdlp_command() (~60줄)
│   └── yt-dlp 35+ 인수 구성
├── spawn_download_process() (~40줄)
│   └── 프로세스 생성 + stdout/stderr 스레드 시작
├── wait_for_process() (~50줄)
│   └── 프로세스 완료 대기 + 진행 이벤트 방출
└── handle_exit_status() (~40줄)
    └── 파일 이동 OR 재시도 OR 실패 처리
```

**단계 4.2: lib.rs 최소화**

`lib.rs`에서 모든 구현 제거 후 남기는 내용:
```rust
// 모듈 선언
mod state;
mod types;
// ... (10개 모듈)

// Tauri 앱 진입점
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(state::SharedState::new(...))
        .invoke_handler(tauri::generate_handler![
            metadata::analyze_url,
            queue::check_duplicate,
            queue::enqueue_job,
            // ... 18개 커맨드 등록
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

### Phase 4 품질 게이트

- [ ] `cargo fmt` — 포맷팅 통과
- [ ] `cargo clippy -- -D warnings` — 경고 0개
- [ ] `cargo build` — 빌드 성공
- [ ] `cargo test` — 모든 특성화 테스트 통과
- [ ] `lib.rs` 줄 수: 80줄 이하
- [ ] 모든 18개 Tauri 커맨드 등록 확인

---

## Phase 5: Polish — 품질 개선 (낮은 위험)

### 목표

문서화, 코드 품질 최종 정리.

### 작업 목록

**단계 5.1: 문서화 추가**

모든 `pub` 항목에 `///` 문서 주석 추가:
- 구조체 필드 설명
- 함수 목적, `# Arguments`, `# Returns`, `# Errors` 섹션
- 18개 Tauri 커맨드 각각 문서화

우선순위 높음:
- `lock_or_recover()` — 다른 모든 모듈이 사용
- `DownloadStatus` — 프론트엔드 인터페이스 타입
- `QueueItem` — 20개 필드 각각 설명
- `start_worker_if_needed()` → 분해된 5개 함수

**단계 5.2: 최종 품질 검사**

```bash
cargo fmt
cargo clippy -- -D warnings
cargo build --release
cargo test
```

**단계 5.3: 성능 회귀 확인**

실제 다운로드 실행으로 기능 검증:
- URL 분석 응답 시간 비교
- 다운로드 시작~완료 플로우 검증
- 큐 조작 (일시정지/재개/취소) 동작 검증

### Phase 5 품질 게이트

- [ ] 모든 `pub` 항목 문서화 완료
- [ ] `cargo doc --no-deps` 오류 없음
- [ ] `cargo clippy -- -D warnings` 경고 0개
- [ ] 통합 다운로드 시나리오 수동 검증

---

## 기술 접근법

### DDD 방법론 적용

```
ANALYZE (완료):
  research.md → 코드 구조, 도메인 경계, 위험 요소 파악

PRESERVE (각 Phase 시작 시):
  특성화 테스트 작성 → 현재 동작을 테스트로 고정
  → "이 테스트가 통과하는 한 동작이 보존되었다"

IMPROVE (각 Phase 중):
  작은 변경 → cargo build → cargo test → 반복
  모든 특성화 테스트를 GREEN 상태로 유지하며 리팩토링
```

### 모듈 의존성 그래프

```
lib.rs (오케스트레이터)
  │
  ├─► state.rs ──────────────── (독립)
  ├─► types.rs ──────────────── (독립)
  ├─► utils.rs ──────────────── types.rs
  │
  ├─► file_ops.rs ─────────────  utils.rs, state.rs
  │
  ├─► dependencies.rs ─────────  file_ops.rs, utils.rs, state.rs
  │
  ├─► diagnostics.rs ──────────  file_ops.rs, dependencies.rs, state.rs
  │
  ├─► settings.rs ─────────────  file_ops.rs, state.rs, utils.rs
  │
  ├─► metadata.rs ─────────────  dependencies.rs
  │
  ├─► queue.rs ────────────────  state.rs, file_ops.rs, download.rs (워커 트리거)
  │
  └─► download.rs ─────────────  queue.rs, dependencies.rs, file_ops.rs,
                                  utils.rs, state.rs
```

순환 의존성 없음 확인.

### 핵심 Rust 패턴

**패턴 1: lock_or_recover 통일**
```rust
// Before (8회 반복):
let mut state = shared.lock().unwrap_or_else(|e| {
    eprintln!("[STABILITY] Mutex poisoned, recovering: {:?}", e);
    e.into_inner()
});

// After:
let mut state = lock_or_recover(&shared, "worker_loop");
```

**패턴 2: DownloadStatus enum 비교**
```rust
// Before:
if item.status == "queued" { ... }

// After:
if item.status == DownloadStatus::Queued { ... }
```

**패턴 3: 함수 분해 (단일 책임 원칙)**
```rust
// Before: 311줄 단일 함수
fn start_worker_if_needed(state: SharedState, app: AppHandle) { /* 모든 것 */ }

// After: 5개 단일 책임 함수
fn poll_next_job(state: &AppState) -> Option<QueueItem> { /* 20줄 */ }
fn build_ytdlp_command(job: &QueueItem) -> Vec<String> { /* 60줄 */ }
// ...
```

---

## 위험 분석 및 대응

| 위험 요소 | 심각도 | 발생 가능성 | 대응 방안 |
|---------|-------|-----------|---------|
| 워커 스레드 동시성 버그 | 높음 | 중간 | Phase 4 마지막 수행, 스레드 안전성 특성화 테스트 |
| 플랫폼 조건부 코드 누락 | 높음 | 낮음 | `#[cfg]` 블록 grep으로 전수 확인 |
| Tauri 커맨드 등록 누락 | 높음 | 낮음 | 18개 커맨드 체크리스트 관리 |
| 뮤텍스 복구 동작 변경 | 중간 | 낮음 | 특성화 테스트로 복구 동작 고정 |
| 순환 의존성 도입 | 중간 | 낮음 | 의존성 그래프 검토 후 진행 |
| `persist_queue` fan_in 변경 | 중간 | 낮음 | 8개 호출 위치 유지 |

---

## 우선순위 마일스톤

### Primary Goal (핵심 목표)
모든 18개 Tauri 커맨드가 기존과 동일하게 동작하는 10개 모듈 구조 완성

달성 기준:
- `lib.rs` 80줄 이하
- `cargo build` 성공
- 모든 특성화 테스트 통과

### Secondary Goal (품질 목표)
코드 품질 지표 개선

달성 기준:
- `cargo clippy -- -D warnings` 경고 0개
- 최장 함수 100줄 이하
- `lock_or_recover` 단일 구현
- `DownloadStatus` enum 완전 적용

### Final Goal (완성 목표)
문서화 및 유지보수성 완성

달성 기준:
- 모든 `pub` 항목 `///` 문서화
- `cargo doc --no-deps` 오류 없음
- 성능 회귀 없음 수동 검증 완료

### Optional Goal (선택적 목표)
테스트 커버리지 확장

달성 기준:
- `cargo test` 커버리지 85% 이상
- 각 모듈별 단위 테스트 존재

---

## 다음 단계

SPEC 승인 후:
1. `/moai:2-run SPEC-REFACTOR-001` 실행
2. DDD 모드: Phase 1부터 순서대로 진행
3. 각 Phase 완료 시 품질 게이트 통과 확인
4. Phase 5 완료 후 `/moai:3-sync SPEC-REFACTOR-001` 실행
