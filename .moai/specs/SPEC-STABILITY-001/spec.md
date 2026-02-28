---
id: SPEC-STABILITY-001
version: 1.0.0
status: draft
created: 2026-03-01
updated: 2026-03-01
author: backgwangmin
priority: critical
---

# SPEC-STABILITY-001: Rust 백엔드 오류 처리 및 안정성 강화

## HISTORY

| 버전 | 날짜 | 작성자 | 변경 내용 |
|------|------|--------|-----------|
| 1.0.0 | 2026-03-01 | backgwangmin | 최초 작성 - 연구 결과 기반 4가지 치명적 이슈 식별 |

## Background

TubeExtract(yt-downloder)는 Tauri 2.0 기반 데스크톱 애플리케이션으로, React/TypeScript 프론트엔드와 Rust 백엔드로 구성된다. 안정성 연구(`research-stability.md`) 결과, Rust 백엔드에서 앱 즉시 충돌을 유발하는 4가지 치명적(Critical) 이슈가 발견되었다.

이 이슈들은 모두 `src-tauri/src/lib.rs` 파일에 집중되어 있으며, 프로덕션 환경에서 연쇄 장애를 유발할 수 있는 수준이다. 본 SPEC은 이 4가지 치명적 이슈와 2가지 높은 우선순위 이슈를 해결하는 것을 목표로 한다.

### 해결 대상 이슈

| 우선순위 | 이슈 | 파일 위치 | 영향 |
|----------|------|-----------|------|
| Critical | Mutex Poisoning 패닉 | `src-tauri/src/lib.rs:1034, 1155, 1205, 1248, 1310, 1340, 1372, 1877` | 앱 즉시 충돌 |
| Critical | 스레드 종료 보장 없음 | `src-tauri/src/lib.rs:1170-1388` | 리소스 고갈 |
| Critical | 타임아웃 미구현 | `src-tauri/src/lib.rs:402-436` | 무한 블로킹 |
| Critical | 시작 실패 하드 패닉 | `src-tauri/src/lib.rs:2061` | 초기화 실패 시 크래시 |
| High | 프로세스 종료 미검증 | `src-tauri/src/lib.rs:1124-1138` | 좀비 프로세스 |
| High | React Query 재시도 설정 | `src/renderer/lib/queryClient.ts:3-10` | 네트워크 복원력 저하 |

### 참고 문서

- 연구 보고서: `/Users/backgwangmin/Documents/yt-downloder/.moai/specs/research-stability.md`
- 기술 스택: Tauri 2.0, Rust 1.70+, React 19, TypeScript

---

## Environment (환경)

### 실행 환경
- **운영체제**: Windows 10/11, macOS 12+, Linux (Ubuntu 20.04+)
- **런타임**: Tauri 2.0 + Rust 1.70+
- **프론트엔드**: React 19 + TypeScript + Tanstack Query (React Query)
- **외부 의존성**: yt-dlp, ffmpeg (런타임 다운로드)

### 제약 조건
- Tauri 2.0 API와의 하위 호환성 유지 필수
- 기존 Tauri IPC 명령 시그니처 변경 금지 (프론트엔드 계약 유지)
- Rust 표준 라이브러리(`std::sync`, `std::thread`, `std::sync::mpsc`)만 사용 (외부 크레이트 추가 최소화)
- 기존 큐(Queue) 상태 직렬화 형식 유지

---

## Assumptions (가정)

| 번호 | 가정 | 신뢰도 | 검증 방법 |
|------|------|--------|-----------|
| A1 | Mutex 독점 시간이 짧아 `recover()` 전략으로 복구 가능하다 | High | Mutex 잠금 패턴 코드 검토 |
| A2 | yt-dlp/ffmpeg 타임아웃은 최소 15초 이상이 적절하다 | Medium | 기존 `_timeout_ms` 호출 지점 파라미터 값 확인 |
| A3 | 워커 스레드는 단일 인스턴스로 운영된다 | High | `lib.rs` 초기화 코드 확인 |
| A4 | Tauri 초기화 실패 시 사용자에게 UI 다이얼로그 표시 가능하다 | Medium | Tauri 2.0 early init dialog API 확인 필요 |
| A5 | TypeScript queryClient 설정 변경은 하위 호환 가능하다 | High | React Query API 문서 확인 |

---

## Requirements (요구사항)

### REQ-001: Mutex Poisoning 복구 처리

**유형**: Unwanted Behavior (불필요한 동작 금지)

시스템은 Mutex 잠금 실패(poisoned) 시 `panic!`을 호출하지 않아야 한다.

**현재 코드 패턴 (수정 대상):**
```rust
// src-tauri/src/lib.rs lines: 1034, 1155, 1205, 1248, 1310, 1340, 1372, 1877
let mut state = shared.lock().unwrap_or_else(|_| panic!("state lock poisoned"));
```

**EARS 요구사항:**

- **[REQ-001-A]** If Mutex::lock()이 PoisonError를 반환하면, the system shall 패닉을 발생시키지 않고 독점된 잠금(poisoned guard)을 `into_inner()`로 복구하여 계속 동작해야 한다.
- **[REQ-001-B]** While Mutex 복구가 발생하는 동안, the system shall 복구 이벤트를 로그에 기록하고 사용자에게 오류 상태를 Tauri 이벤트로 알려야 한다.
- **[REQ-001-C]** 시스템은 항상 Mutex 복구 후 데이터 일관성을 확인하여 손상된 상태가 전파되지 않도록 해야 한다.

---

### REQ-002: 워커 스레드 종료 보장

**유형**: Event-Driven + State-Driven

**EARS 요구사항:**

- **[REQ-002-A]** When 애플리케이션 종료 신호가 발생하면, the system shall 워커 스레드에게 종료 신호를 채널(`std::sync::mpsc::channel`)을 통해 전달하고 스레드가 정상 종료될 때까지 대기해야 한다.
- **[REQ-002-B]** While 워커 스레드가 실행 중인 동안, the system shall stdout/stderr 리더 스레드의 핸들을 추적하고 부모 프로세스 종료 시 함께 정리해야 한다.
- **[REQ-002-C]** If 워커 스레드가 종료 신호 수신 후 5초 이내에 종료되지 않으면, the system shall 스레드를 강제로 종료하고 이를 오류 로그에 기록해야 한다.
- **[REQ-002-D]** 시스템은 항상 생성된 스레드의 JoinHandle을 보관하여 패닉으로 인한 스레드 실패를 감지할 수 있어야 한다.

---

### REQ-003: 명령 실행 타임아웃 구현

**유형**: Event-Driven + Unwanted Behavior

**현재 코드 패턴 (수정 대상):**
```rust
// src-tauri/src/lib.rs lines: 402-436
fn run_command_capture(
    app: &AppHandle,
    command: &str,
    args: &[&str],
    _timeout_ms: u64,  // UNUSED - 실제로 타임아웃이 적용되지 않음
) -> CommandCaptureResult { ... }
```

**EARS 요구사항:**

- **[REQ-003-A]** When `run_command_capture`가 호출되면, the system shall `_timeout_ms` 파라미터로 지정된 시간 내에 명령이 완료되지 않으면 프로세스를 강제 종료해야 한다.
- **[REQ-003-B]** If 타임아웃이 발생하면, the system shall `CommandCaptureResult`에 타임아웃 오류를 반환하고 caller에게 타임아웃 발생 사실을 명시적으로 알려야 한다.
- **[REQ-003-C]** 시스템은 항상 타임아웃 구현에 `std::thread::spawn` + watchdog 패턴 또는 `Child::wait_timeout`을 사용하여 워커 스레드를 블로킹하지 않아야 한다.
- **[REQ-003-D]** Where 타임아웃이 0으로 설정된 경우, the system shall 타임아웃 없이 무제한 실행을 허용해야 한다 (기존 동작과의 하위 호환성).

---

### REQ-004: 시작 실패 사용자 친화적 처리

**유형**: Unwanted Behavior + Event-Driven

**현재 코드 패턴 (수정 대상):**
```rust
// src-tauri/src/lib.rs line: 2061
.run(tauri::generate_context!())
.expect("error while running tauri application");  // 하드 패닉
```

**EARS 요구사항:**

- **[REQ-004-A]** If Tauri 애플리케이션 초기화가 실패하면, the system shall `expect()`를 사용하지 않고 오류를 로그에 기록한 후 사용자에게 친화적인 오류 메시지를 표시해야 한다.
- **[REQ-004-B]** When Tauri 시작 오류가 발생하면, the system shall 오류 유형(권한 부족, 포트 충돌, 리소스 부족 등)을 분류하여 복구 가능 여부를 사용자에게 안내해야 한다.
- **[REQ-004-C]** 시스템은 항상 시작 오류를 구조화된 로그 형식(타임스탬프, 오류 코드, 스택 트레이스)으로 파일에 기록해야 한다.

---

### REQ-005: 프로세스 종료 검증

**유형**: State-Driven + Event-Driven

**EARS 요구사항:**

- **[REQ-005-A]** When `terminate_child_with_grace_period`가 호출되면, the system shall 500ms 그레이스 피리어드 후 프로세스가 실제로 종료되었는지 검증해야 한다.
- **[REQ-005-B]** While Windows 환경에서 프로세스 종료를 시도하는 동안, the system shall `taskkill /F /T` 명령을 사용하여 자식 프로세스(yt-dlp가 spawn한 하위 프로세스)도 함께 종료해야 한다.
- **[REQ-005-C]** If 프로세스가 종료 후에도 계속 실행 중이면, the system shall 해당 상태를 오류 로그에 기록하고 사용자에게 수동 종료를 안내해야 한다.

---

### REQ-006: React Query 복원력 설정

**유형**: Ubiquitous (항상 적용)

**현재 코드 패턴 (수정 대상):**
```typescript
// src/renderer/lib/queryClient.ts lines: 3-10
// 현재: retry: 1 (불충분)
```

**EARS 요구사항:**

- **[REQ-006-A]** 시스템은 항상 React Query 클라이언트의 재시도 횟수를 최소 3회로 설정하여 일시적인 네트워크 오류에 대한 복원력을 제공해야 한다.
- **[REQ-006-B]** 시스템은 항상 `staleTime`을 30,000ms(30초)로 설정하여 불필요한 백그라운드 재조회를 방지해야 한다.
- **[REQ-006-C]** Where React Query 재시도가 설정된 경우, the system shall 지수 백오프(exponential backoff) 전략을 적용하여 백엔드에 과부하를 주지 않아야 한다.

---

## Specifications (명세)

### SPEC-001: Mutex 복구 패턴

**대상 파일**: `src-tauri/src/lib.rs`
**대상 라인**: 1034, 1155, 1205, 1248, 1310, 1340, 1372, 1877

수정 전:
```rust
let mut state = shared.lock().unwrap_or_else(|_| panic!("state lock poisoned"));
```

수정 후 패턴:
```rust
let mut state = shared.lock().unwrap_or_else(|e| {
    log::error!("Mutex poisoned, recovering: {:?}", e);
    e.into_inner()
});
```

모든 8개 발생 지점에 동일한 패턴 적용.

---

### SPEC-002: 워커 스레드 종료 채널

**대상 파일**: `src-tauri/src/lib.rs`
**대상 라인**: 1170-1388

- `std::sync::mpsc::channel::<()>()` 를 사용하여 shutdown sender/receiver 쌍 생성
- 워커 루프에서 `try_recv()` 를 주기적으로 폴링하여 종료 신호 수신
- 애플리케이션 종료 시 sender를 통해 `()` 메시지 전송
- `JoinHandle`을 State에 보관하여 추적

---

### SPEC-003: 타임아웃 Watchdog 구현

**대상 파일**: `src-tauri/src/lib.rs`
**대상 함수**: `run_command_capture` (lines 402-436)

- `_timeout_ms` 파라미터 이름을 `timeout_ms`로 변경 (경고 제거)
- `timeout_ms > 0`인 경우 별도 스레드에서 타임아웃 감시
- `Child::wait_timeout(Duration::from_millis(timeout_ms))` 패턴 사용 권장
- 타임아웃 발생 시 `CommandCaptureResult::Timeout` variant 반환

---

### SPEC-004: Tauri 시작 오류 처리

**대상 파일**: `src-tauri/src/lib.rs`
**대상 라인**: 2061

- `.expect()` 를 `match` 또는 `if let Err(e)` 패턴으로 교체
- Tauri 빌더의 오류를 `tauri::Error` 유형으로 처리
- 가능한 경우 `tauri::api::dialog::message()` 또는 네이티브 다이얼로그로 사용자에게 알림
- 오류 세부 정보는 앱 로그 디렉토리에 기록

---

### SPEC-005: TypeScript queryClient 설정

**대상 파일**: `src/renderer/lib/queryClient.ts`
**대상 라인**: 3-10

```typescript
// 수정 후
const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      retry: 3,
      staleTime: 30_000,
      retryDelay: (attemptIndex) => Math.min(1000 * 2 ** attemptIndex, 30_000),
    },
    mutations: {
      retry: 1,
    },
  },
});
```

---

## Traceability (추적성)

| 요구사항 ID | 연구 보고서 이슈 | 구현 파일 | 수락 기준 |
|-------------|-----------------|-----------|-----------|
| REQ-001 | Issue #1 (Mutex Poisoning) | `src-tauri/src/lib.rs` | AC-001 |
| REQ-002 | Issue #2 (Thread Spawning) | `src-tauri/src/lib.rs` | AC-002 |
| REQ-003 | Issue #3 (Timeout) | `src-tauri/src/lib.rs` | AC-003 |
| REQ-004 | Issue #4 (Startup Panic) | `src-tauri/src/lib.rs` | AC-004 |
| REQ-005 | Issue #6 (Process Termination) | `src-tauri/src/lib.rs` | AC-005 |
| REQ-006 | Issue #10 (React Query) | `src/renderer/lib/queryClient.ts` | AC-006 |

<!-- TAG: SPEC-STABILITY-001 -->
