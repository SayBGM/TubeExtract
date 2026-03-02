---
id: SPEC-REFACTOR-001
version: 0.1.0
status: draft
created: 2026-03-01
updated: 2026-03-01
author: backgwangmin
priority: medium
domain: REFACTOR
methodology: ddd
---

# SPEC-REFACTOR-001: lib.rs 모노리스 모듈 분리 리팩토링

## 환경 (Environment)

### 프로젝트 컨텍스트

- **프로젝트**: TubeExtract (yt-downloader) — Tauri 2.0 데스크톱 앱
- **기술 스택**: TypeScript (프론트엔드) + Rust (백엔드, `src-tauri/src/`)
- **현재 상태**: `src-tauri/src/lib.rs` 단일 파일 2,491줄에 모든 비즈니스 로직 집중
- **목표 상태**: 10개 모듈로 분리, `lib.rs` 80줄 이하
- **개발 방법론**: DDD (ANALYZE-PRESERVE-IMPROVE) — `quality.yaml` 설정 기준

### 현재 아키텍처 문제

| 지표 | 현재 값 | 목표 값 |
|------|--------|--------|
| `lib.rs` 총 줄 수 | 2,491 줄 | ~80 줄 |
| 최장 함수 길이 | 311 줄 (`start_worker_if_needed`) | 100 줄 이하 |
| 중복 뮤텍스 복구 패턴 | 8회 반복 | 1개 헬퍼 함수 |
| 매직 상태 문자열 | 20+ 곳 | 0 (타입 안전 enum) |
| 공개 항목 문서화 | 0% | 100% |
| Tauri 커맨드 수 | 18개 | 18개 (동일, 동작 보존) |

---

## 가정 (Assumptions)

1. **동작 보존 최우선**: 리팩토링 전후 18개 Tauri 커맨드의 외부 API 시그니처와 동작이 동일하게 유지되어야 한다.
2. **하위 호환성**: TypeScript 프론트엔드 코드 수정 없이 Rust 모듈 분리가 완료되어야 한다.
3. **점진적 추출**: 각 Phase 완료 후 `cargo build`와 기능 검증이 통과해야 다음 Phase로 진행한다.
4. **순환 의존성 없음**: 제안된 모듈 의존 그래프에 순환 의존이 없으며 이를 유지한다.
5. **플랫폼 코드 보존**: `#[cfg(target_os)]` 조건부 컴파일 블록은 기존과 동일하게 보존한다.
6. **테스트 우선 접근**: DDD 방법론에 따라 각 모듈 추출 전에 해당 도메인의 특성화 테스트(Characterization Test)를 작성한다.

---

## 요구사항 (Requirements)

### REQ-001: 모듈 구조 재편성

**The system shall** `src-tauri/src/lib.rs`를 10개의 독립 모듈 파일로 분리하여 각 파일이 단일 책임 원칙을 따르도록 해야 한다.

**대상 모듈 구조**:
```
src-tauri/src/
├── lib.rs             (~80줄: mod 선언 + run() + 커맨드 등록만)
├── state.rs           (~50줄: SharedState, lock_or_recover 헬퍼)
├── types.rs           (~80줄: DownloadStatus enum, CommandResult)
├── utils.rs           (~80줄: 순수 파싱 헬퍼 함수)
├── file_ops.rs        (~150줄: 원자적 파일 이동, 경로 해석, 커맨드 실행)
├── dependencies.rs    (~200줄: 부트스트랩, 버전 확인, ffmpeg 설치)
├── diagnostics.rs     (~150줄: 시스템 진단, 스토리지 통계, 폴더/URL 열기)
├── settings.rs        (~100줄: AppSettings, 복구 로직, 영속성)
├── metadata.rs        (~180줄: analyze_url, 품질 옵션 파싱)
├── queue.rs           (~200줄: QueueItem, 큐 커맨드, 스냅샷, 영속성)
└── download.rs        (~350줄: 워커 스레드, 재시도 로직, 프로세스 관리)
```

### REQ-002: 뮤텍스 복구 패턴 중앙화

**When** 뮤텍스 잠금이 필요할 때, **the system shall** `lock_or_recover()` 헬퍼 함수를 통해 단일 구현으로 처리해야 한다.

```rust
// 목표 패턴
fn lock_or_recover<T>(mutex: &Arc<Mutex<T>>, context: &str) -> MutexGuard<'_, T>
```

**현재 문제**: 아래 패턴이 8개소에 복붙되어 있음:
```rust
let mut state = shared.lock().unwrap_or_else(|e| {
    eprintln!("[STABILITY] Mutex poisoned, recovering: {:?}", e);
    e.into_inner()
});
```

### REQ-003: DownloadStatus 타입 안전 Enum 도입

**When** 다운로드 상태를 비교하거나 설정할 때, **the system shall** 문자열 리터럴 대신 `DownloadStatus` enum을 사용해야 한다.

**대체 대상 매직 문자열**:
- `"queued"` → `DownloadStatus::Queued`
- `"downloading"` → `DownloadStatus::Downloading`
- `"paused"` → `DownloadStatus::Paused`
- `"canceled"` → `DownloadStatus::Canceled`
- `"completed"` → `DownloadStatus::Completed`
- `"failed"` → `DownloadStatus::Failed`

**The system shall** `DownloadStatus`가 `serde::{Serialize, Deserialize}` 및 `Display`를 구현하여 프론트엔드와의 JSON 직렬화 호환성을 유지해야 한다.

### REQ-004: start_worker_if_needed() 함수 분해

**While** 다운로드 워커가 실행 중일 때, **the system shall** 워커 함수를 다음과 같은 단일 책임 하위 함수들로 분해해야 한다:

- `poll_next_job()` — 큐에서 다음 작업 추출
- `build_ytdlp_command()` — yt-dlp 커맨드 인수 구성 (35+ 인수)
- `spawn_download_process()` — 프로세스 생성 및 stdout/stderr 스레드 시작
- `wait_for_process()` — 프로세스 완료 대기 및 진행 이벤트 처리
- `handle_exit_status()` — 종료 상태에 따른 파일 이동 및 재시도 판단

**각 함수는 100줄을 초과하지 않아야 한다.**

### REQ-005: 동작 보존 검증

**When** 각 Phase의 리팩토링이 완료될 때, **the system shall** 모든 18개 Tauri 커맨드가 이전과 동일한 응답을 반환함을 검증해야 한다.

**If** `cargo test`, `cargo clippy`, 또는 `cargo build`가 실패하면, **the system shall** 다음 Phase로 진행하지 않아야 한다.

### REQ-006: 문서화 추가

**The system shall** 모든 공개(pub) 함수, 구조체, enum에 `///` 문서 주석을 추가해야 한다.

**Where** Tauri 커맨드가 존재하는 경우, **the system shall** 다음을 문서화해야 한다:
- 함수 목적
- 매개변수 설명
- 반환 값 및 오류 케이스

### REQ-007: 품질 게이트 통과

**The system shall** 각 Phase 완료 시 다음 품질 게이트를 통과해야 한다:

- `cargo fmt` — 포맷팅 일관성
- `cargo clippy -- -D warnings` — 경고 없음
- `cargo build` — 빌드 성공
- `cargo test` — 모든 테스트 통과 (특성화 테스트 포함)

---

## 명세 (Specifications)

### SPEC-001: state.rs — 공유 상태 및 복구 헬퍼

**목적**: Tauri 앱 전반에서 공유되는 뮤텍스 래핑 상태 및 복구 유틸리티

**포함 항목**:
- `AppState` 구조체 (큐, 설정, 런타임 상태 보유)
- `SharedState` 타입 별칭 (`Arc<Mutex<AppState>>`)
- `SharedRuntime` 타입 별칭
- `lock_or_recover<T>()` 헬퍼 함수

**의존성**: 없음 (순수 타입/헬퍼)

### SPEC-002: types.rs — 도메인 타입 정의

**목적**: 전체 시스템에서 사용하는 핵심 타입 정의

**포함 항목**:
- `DownloadStatus` enum (6개 변형)
- `CommandResult<T>` 타입 별칭
- 공유 에러 타입

**직렬화 요구사항**:
```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum DownloadStatus {
    Queued,
    Downloading,
    Paused,
    Canceled,
    Completed,
    Failed,
}
```

**의존성**: 없음

### SPEC-003: utils.rs — 순수 파싱 헬퍼

**목적**: 사이드 이펙트 없는 순수 함수들

**포함 항목**:
- `parse_progress_percent(line: &str) -> Option<f64>`
- `parse_speed(line: &str) -> Option<String>`
- `parse_eta(line: &str) -> Option<String>`
- `append_download_log(job_id: &str, line: &str)`
- `select_format_expression(mode: &DownloadMode) -> String`
- `expected_extension(mode: &DownloadMode) -> &str`
- `sanitize_file_name(name: &str) -> String`
- `normalize_youtube_video_url(url: &str) -> String`

**의존성**: `types.rs`

### SPEC-004: file_ops.rs — 파일 및 프로세스 작업

**목적**: 원자적 파일 작업, 경로 해석, 외부 프로세스 실행

**포함 항목**:
- `write_atomic(path: &Path, content: &[u8])` — 원자적 파일 쓰기
- `move_file_atomic(src: &Path, dst: &Path)` — 원자적 파일 이동
- `resolve_downloaded_file_path(...)` — 다운로드 파일 경로 탐색
- `resolve_executable(name: &str) -> Option<PathBuf>` — 실행 파일 탐색
- `run_command_capture(...)` — 외부 커맨드 실행 및 출력 캡처 (126줄 → 분해)
- 경로 헬퍼: `app_data_dir()`, `temp_downloads_root_dir()`, `queue_file_path()`, 등

**run_command_capture 분해 목표**:
- `build_command(name: &str, args: &[&str]) -> Command`
- `run_with_watchdog(cmd: Command, timeout_ms: u64) -> Result<Output>`

**의존성**: `utils.rs`, `state.rs`

### SPEC-005: dependencies.rs — 의존성 관리

**목적**: yt-dlp 및 ffmpeg 부트스트랩, 버전 확인, 플랫폼별 설치

**포함 항목**:
- `bootstrap_dependencies(state: SharedState, app: AppHandle)` — 진입점
- `ensure_ytdlp(...)` — yt-dlp 설치 확인 및 업데이트
- `ensure_ffmpeg_available(...)` — ffmpeg 가용성 확인
- `install_ffmpeg_windows(...)` — Windows 전용 설치 (플랫폼 조건부)
- `latest_ytdlp_version()` — GitHub API에서 최신 버전 확인
- `download_file(url: &str, path: &Path)` — 파일 다운로드
- `wait_for_dependencies(state: SharedState)` — 준비 완료 대기

**의존성**: `file_ops.rs`, `utils.rs`, `state.rs`

### SPEC-006: diagnostics.rs — 시스템 진단

**목적**: 시스템 상태 확인, 스토리지 통계, 외부 리소스 열기

**포함 항목**:
- `run_diagnostics(state: SharedState, app: AppHandle)` — Tauri 커맨드
- `get_storage_stats(state: SharedState)` — Tauri 커맨드
- `open_folder(path: String, app: AppHandle)` — Tauri 커맨드 (플랫폼 분기 개선)
- `open_external_url(url: String, app: AppHandle)` — Tauri 커맨드
- `check_update()` — Tauri 커맨드 (stub)
- `can_write_to_dir(path: &Path) -> bool`
- `calculate_directory_size(path: &Path) -> u64`

**open_folder/open_external_url 중복 제거**: 플랫폼 분기 로직을 `open_with_platform_command(target: &str)` 공통 함수로 추출

**의존성**: `file_ops.rs`, `dependencies.rs`, `state.rs`

### SPEC-007: settings.rs — 설정 및 영속성

**목적**: 앱 설정 로드, 저장, 복구 로직

**포함 항목**:
- `AppSettings` 구조체
- `PersistedSettings` 구조체
- `load_settings_with_recovery(app: &AppHandle) -> AppSettings` — 3단계 폴백
- `load_queue_with_recovery(app: &AppHandle) -> Vec<QueueItem>` — 3단계 폴백
- `persist_settings(settings: &AppSettings, app: &AppHandle)`
- `normalize_download_dir(dir: &str, app: &AppHandle) -> PathBuf`

**복구 폴백 패턴** (중복 제거 대상):
```
1차 시도: 주 파일 로드
2차 시도: 백업 파일 로드
3차 시도: 기본값 사용
```

**의존성**: `file_ops.rs`, `state.rs`, `utils.rs`

### SPEC-008: metadata.rs — URL 분석 및 품질 옵션

**목적**: YouTube URL 분석, 포맷 파싱, 품질 옵션 추출

**포함 항목**:
- `QualityOption` 구조체
- `AnalysisResult` 구조체
- `DownloadMode` enum
- `analyze_url(url: String, state: SharedState, app: AppHandle)` — Tauri 커맨드 (165줄 → 분해)
- `normalize_youtube_video_url(url: &str) -> String`

**analyze_url 분해 목표**:
- `fetch_metadata_json(url: &str) -> Result<serde_json::Value>` — yt-dlp 실행
- `parse_quality_options(json: &serde_json::Value) -> Vec<QualityOption>` — 포맷 파싱
- `build_analysis_result(json: &serde_json::Value, options: Vec<QualityOption>) -> AnalysisResult`

**의존성**: `dependencies.rs`

### SPEC-009: queue.rs — 큐 관리

**목적**: 다운로드 큐 상태, 커맨드, 영속성

**포함 항목**:
- `QueueItem` 구조체 (20개 필드, `DownloadStatus` 타입 사용)
- `enqueue_job(...)` — Tauri 커맨드
- `check_duplicate(url: &str, quality: &str, state: SharedState)` — Tauri 커맨드
- `cancel_job(job_id: String, state: SharedState, app: AppHandle)` — Tauri 커맨드
- `pause_job(...)`, `resume_job(...)` — Tauri 커맨드
- `clear_terminal_jobs(...)` — Tauri 커맨드
- `delete_file(...)` — Tauri 커맨드
- `get_queue_snapshot(state: SharedState)` — Tauri 커맨드
- `persist_queue(queue: &[QueueItem], app: &AppHandle)` — fan_in=8 유지
- `scan_incomplete_markers(dir: &Path) -> Vec<PathBuf>`
- `build_unique_output_path(dir: &Path, name: &str, ext: &str) -> PathBuf`

**의존성**: `state.rs`, `file_ops.rs`, `download.rs` (워커 트리거)

### SPEC-010: download.rs — 다운로드 워커

**목적**: 다운로드 실행, 재시도 로직, 프로세스 생명주기 관리

**포함 항목**:
- `RetryStrategy` enum
- `ActiveProcess` 구조체
- `start_worker_if_needed(state: SharedState, app: AppHandle)` — 진입점 (311줄 → 분해)
  - `poll_next_job(state: &AppState) -> Option<QueueItem>`
  - `build_ytdlp_command(job: &QueueItem, paths: &DownloadPaths) -> Vec<String>`
  - `spawn_download_process(cmd: Vec<String>, ...) -> ActiveProcess`
  - `wait_for_process(process: ActiveProcess, ...) -> ExitStatus`
  - `handle_exit_status(status: ExitStatus, job: &QueueItem, ...) -> WorkerAction`
- `classify_download_error(stderr: &str) -> RetryStrategy`
- `retry_delay_ms_for_strategy(strategy: &RetryStrategy, attempt: u32) -> u64`
- `handle_download_output_line(line: &str, job_id: &str, app: &AppHandle)`
- `kill_active_child_unchecked(process: &mut ActiveProcess)`
- `terminate_child_with_grace_period(process: &mut ActiveProcess)`

**의존성**: `queue.rs`, `dependencies.rs`, `file_ops.rs`, `utils.rs`, `state.rs`

### SPEC-011: lib.rs — 최소화된 진입점

**목적**: 모듈 선언과 Tauri 앱 실행 함수만 포함

**포함 항목**:
```rust
// 모듈 선언 (~10줄)
mod state;
mod types;
mod utils;
mod file_ops;
mod dependencies;
mod diagnostics;
mod settings;
mod metadata;
mod queue;
mod download;

// run() 함수 (~30줄): Tauri 빌더 + 커맨드 등록 + 상태 초기화
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() { ... }
```

**목표 줄 수**: 80줄 이하

---

## 추적성 (Traceability)

| 요구사항 | 관련 모듈 | 품질 기준 |
|---------|---------|---------|
| REQ-001 | SPEC-001 ~ SPEC-011 | 10개 파일 생성, lib.rs < 80줄 |
| REQ-002 | SPEC-001 (state.rs) | lock_or_recover 1회 구현 |
| REQ-003 | SPEC-002 (types.rs) | DownloadStatus enum 도입 |
| REQ-004 | SPEC-010 (download.rs) | 함수 분해, 각 < 100줄 |
| REQ-005 | 전체 | 18개 커맨드 동작 동일 |
| REQ-006 | 전체 | pub 항목 100% 문서화 |
| REQ-007 | 전체 | cargo 품질 게이트 통과 |
