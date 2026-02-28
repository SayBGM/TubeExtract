---
id: SPEC-STABILITY-001
document: acceptance
version: 1.0.0
---

# SPEC-STABILITY-001 수락 기준

<!-- TAG: SPEC-STABILITY-001 -->

## 개요

본 문서는 SPEC-STABILITY-001의 모든 요구사항에 대한 검증 기준과 테스트 시나리오를 정의한다. 각 시나리오는 Given-When-Then 형식으로 작성되며, DDD 방법론의 PRESERVE/IMPROVE 단계에서 활용된다.

---

## AC-001: Mutex Poisoning 복구 처리

**관련 요구사항**: REQ-001 (SPEC-STABILITY-001)

### 시나리오 AC-001-1: Mutex 독점 시 패닉 없이 복구

**Given** Rust 백엔드 워커 스레드가 Shared State의 Mutex를 보유한 채 패닉하여 Mutex가 poisoned 상태가 되었을 때,

**When** 다른 스레드가 해당 Mutex에 대해 `.lock()`을 호출하면,

**Then**
- 애플리케이션이 `panic!`으로 충돌하지 않아야 한다
- `into_inner()`를 통해 내부 데이터에 접근해야 한다
- 오류 로그에 `"Mutex poisoned"` 관련 메시지가 기록되어야 한다
- 이후 정상 작업(큐 처리 등)이 계속 진행되어야 한다

### 시나리오 AC-001-2: Mutex 복구 후 데이터 일관성

**Given** Mutex가 poisoned 상태에서 `into_inner()`로 복구되었을 때,

**When** 복구된 상태 데이터를 읽거나 수정하면,

**Then**
- 복구된 데이터를 기반으로 후속 작업이 정상 처리되거나
- 손상된 상태가 감지된 경우 해당 작업만 실패 처리(앱 전체는 계속 실행)되어야 한다
- Tauri 이벤트를 통해 프론트엔드에 오류 상태가 전달되어야 한다

### 검증 방법

```rust
// Rust 단위 테스트
#[test]
fn test_mutex_poison_recovery() {
    let shared = Arc::new(Mutex::new(0u32));
    let shared_clone = Arc::clone(&shared);

    // 스레드가 Mutex를 보유한 채 패닉
    let _ = std::thread::spawn(move || {
        let _guard = shared_clone.lock().unwrap();
        panic!("intentional panic to poison mutex");
    }).join();

    // 이 시점에서 Mutex는 poisoned 상태
    assert!(shared.lock().is_err());

    // 복구 코드 - panic이 발생하지 않아야 함
    let recovered = shared.lock().unwrap_or_else(|e| e.into_inner());
    assert_eq!(*recovered, 0); // 데이터 접근 가능
}
```

---

## AC-002: 워커 스레드 종료 보장

**관련 요구사항**: REQ-002 (SPEC-STABILITY-001)

### 시나리오 AC-002-1: 정상 종료 신호 전달

**Given** 워커 스레드가 실행 중이고 mpsc 채널을 통한 종료 신호 메커니즘이 구현되어 있을 때,

**When** 애플리케이션 종료 이벤트가 발생하여 shutdown sender에 `()` 메시지를 전송하면,

**Then**
- 워커 스레드가 현재 처리 중인 작업을 완료한 후 루프를 종료해야 한다
- 스레드 종료가 5초 이내에 이루어져야 한다
- `JoinHandle.join()`이 `Ok(())`를 반환해야 한다
- 로그에 `"Shutdown signal received"` 메시지가 기록되어야 한다

### 시나리오 AC-002-2: stdout/stderr 리더 스레드 정리

**Given** yt-dlp 프로세스 실행 중에 stdout/stderr 리더 스레드가 생성되어 있을 때,

**When** 해당 다운로드 작업이 완료되거나 취소되면,

**Then**
- stdout 리더 스레드가 프로세스 종료 후 자동으로 종료되어야 한다
- stderr 리더 스레드가 프로세스 종료 후 자동으로 종료되어야 한다
- 스레드 핸들이 State에서 제거되어야 한다

### 시나리오 AC-002-3: 종료 타임아웃 처리

**Given** 워커 스레드가 종료 신호를 받았지만 5초 이상 응답하지 않는 상황일 때,

**When** 5초 타임아웃이 경과하면,

**Then**
- 애플리케이션이 무한 대기 없이 종료 프로세스를 계속해야 한다
- 오류 로그에 타임아웃 발생 사실이 기록되어야 한다

### 검증 방법

```rust
#[test]
fn test_worker_shutdown_via_channel() {
    let (shutdown_tx, shutdown_rx) = std::sync::mpsc::channel::<()>();

    let handle = std::thread::spawn(move || {
        loop {
            if shutdown_rx.try_recv().is_ok() {
                break; // 종료 신호 수신
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    });

    // 종료 신호 전송
    shutdown_tx.send(()).unwrap();

    // 5초 이내 종료 확인
    let result = handle.join();
    assert!(result.is_ok());
}
```

---

## AC-003: 명령 실행 타임아웃 강제

**관련 요구사항**: REQ-003 (SPEC-STABILITY-001)

### 시나리오 AC-003-1: 타임아웃 내 명령 완료

**Given** `run_command_capture`가 `timeout_ms: 5000`으로 호출되고 명령이 3초 내에 완료될 때,

**When** 명령이 3초 후 정상 종료되면,

**Then**
- `CommandCaptureResult::Success` (또는 동등한 성공 variant)가 반환되어야 한다
- 타임아웃이 발동하지 않아야 한다
- 프로세스가 정상 종료 상태 코드를 반환해야 한다

### 시나리오 AC-003-2: 타임아웃 초과 시 강제 종료

**Given** `run_command_capture`가 `timeout_ms: 2000`으로 호출되고 명령이 10초 이상 걸리는 상황일 때,

**When** 2초 타임아웃이 경과하면,

**Then**
- 명령 프로세스가 강제 종료(kill)되어야 한다
- `CommandCaptureResult::Timeout` (또는 동등한 타임아웃 variant)이 반환되어야 한다
- 워커 스레드가 블로킹 없이 다음 작업을 처리할 수 있어야 한다
- 로그에 타임아웃 발생 및 프로세스 종료 사실이 기록되어야 한다

### 시나리오 AC-003-3: timeout_ms = 0 시 무제한 실행 (하위 호환)

**Given** `run_command_capture`가 `timeout_ms: 0`으로 호출될 때,

**When** 명령이 실행되면,

**Then**
- 타임아웃 없이 명령이 완료될 때까지 대기해야 한다
- 기존 동작과 동일하게 처리되어야 한다

### 검증 방법

```rust
#[test]
fn test_timeout_enforced() {
    // 30초 sleep 명령으로 타임아웃 테스트
    let result = run_command_capture(
        &app_handle,
        "sleep",
        &["30"],
        2000, // 2초 타임아웃
    );
    assert!(matches!(result, CommandCaptureResult::Timeout));
}

#[test]
fn test_timeout_zero_means_unlimited() {
    // 빠른 명령으로 timeout_ms=0 테스트
    let result = run_command_capture(
        &app_handle,
        "echo",
        &["hello"],
        0, // 무제한
    );
    assert!(matches!(result, CommandCaptureResult::Success(_)));
}
```

---

## AC-004: 시작 실패 사용자 친화적 처리

**관련 요구사항**: REQ-004 (SPEC-STABILITY-001)

### 시나리오 AC-004-1: Tauri 초기화 오류 시 우아한 종료

**Given** Tauri 애플리케이션 빌더에서 초기화 오류(예: 포트 충돌, 권한 없음)가 발생할 때,

**When** `builder.run()`이 `Err(e)`를 반환하면,

**Then**
- 애플리케이션이 `panic!` 또는 uncaught exception으로 종료하지 않아야 한다
- 오류 세부 정보가 앱 로그 디렉토리의 파일에 기록되어야 한다
- 사용자에게 이해 가능한 오류 메시지가 표시되거나 (다이얼로그 또는 stderr)
- `std::process::exit(1)`로 정상 종료 코드가 설정되어야 한다

### 시나리오 AC-004-2: 시작 오류 로그 구조화

**Given** Tauri 시작 오류가 발생했을 때,

**When** 오류가 처리되면,

**Then**
- 로그 파일에 타임스탬프가 포함되어야 한다
- 오류 타입 또는 코드가 기록되어야 한다
- 가능한 경우 스택 트레이스 또는 원인 체인이 포함되어야 한다
- 로그 파일이 OS 표준 앱 데이터 디렉토리에 저장되어야 한다

### 검증 방법

Tauri 초기화 실패 시나리오는 통합 테스트 또는 수동 테스트로 검증:
- 포트가 이미 사용 중인 환경에서 앱 실행 시 패닉 없이 오류 메시지 표시 확인
- 오류 로그 파일 생성 및 내용 확인

---

## AC-005: 프로세스 종료 검증

**관련 요구사항**: REQ-005 (SPEC-STABILITY-001)

### 시나리오 AC-005-1: 그레이스 피리어드 후 종료 확인

**Given** yt-dlp 다운로드 프로세스가 실행 중일 때,

**When** 사용자가 다운로드 취소를 요청하여 `terminate_child_with_grace_period`가 호출되면,

**Then**
- 500ms 그레이스 피리어드 후 `child.try_wait()` 또는 동등한 방법으로 프로세스 종료 여부를 확인해야 한다
- 프로세스가 이미 종료되었으면 추가 kill 시도 없이 완료 처리해야 한다
- 프로세스가 여전히 실행 중이면 force kill을 실행해야 한다

### 시나리오 AC-005-2: Windows 자식 프로세스 트리 종료

**Given** Windows 환경에서 yt-dlp가 ffmpeg 같은 자식 프로세스를 spawn하여 실행 중일 때,

**When** 부모 yt-dlp 프로세스를 종료하라는 요청이 발생하면,

**Then**
- `taskkill /F /T /PID {pid}` 명령을 통해 프로세스 트리 전체가 종료되어야 한다
- 취소 후 시스템 프로세스 목록에 고아(orphan) yt-dlp 또는 ffmpeg 프로세스가 남지 않아야 한다

### 시나리오 AC-005-3: 종료 실패 시 사용자 안내

**Given** 프로세스 종료 시도 후에도 프로세스가 계속 실행 중일 때,

**When** 프로세스 종료 실패가 감지되면,

**Then**
- 오류 로그에 종료 실패 및 프로세스 PID가 기록되어야 한다
- 가능한 경우 프론트엔드에 "수동 종료 필요" 오류 이벤트가 전달되어야 한다

---

## AC-006: React Query 복원력 설정

**관련 요구사항**: REQ-006 (SPEC-STABILITY-001)

### 시나리오 AC-006-1: 쿼리 재시도 3회 동작

**Given** React Query가 구성된 queryClient를 사용하고 있을 때,

**When** Tauri IPC 호출이 일시적 오류(네트워크 불안정, 백엔드 일시 중단)로 실패하면,

**Then**
- 쿼리가 자동으로 최대 3회 재시도되어야 한다
- 재시도 간격이 지수 백오프(1s, 2s, 4s, 최대 30s)를 따라야 한다
- 3회 재시도 후에도 실패하면 오류 상태로 전환되어야 한다

### 시나리오 AC-006-2: StaleTime 30초 적용

**Given** queryClient의 defaultOptions에 staleTime: 30000이 설정되어 있을 때,

**When** 이미 로드된 쿼리 데이터에 30초 이내로 다시 접근하면,

**Then**
- 백그라운드 재조회가 발생하지 않아야 한다
- 캐시된 데이터가 즉시 반환되어야 한다
- 30초 이후에는 정상적으로 stale 상태로 전환되어야 한다

### 시나리오 AC-006-3: queryClient 설정 확인

**Given** `src/renderer/lib/queryClient.ts` 파일이 수정되었을 때,

**When** queryClient 인스턴스를 확인하면,

**Then**
- `defaultOptions.queries.retry` 값이 `3`이어야 한다
- `defaultOptions.queries.staleTime` 값이 `30000`이어야 한다
- `defaultOptions.queries.retryDelay`가 지수 백오프 함수로 설정되어야 한다

### 검증 방법

```typescript
// TypeScript 단위 테스트
import { queryClient } from './queryClient';

describe('queryClient configuration', () => {
  it('should have retry set to 3', () => {
    const defaultOptions = queryClient.getDefaultOptions();
    expect(defaultOptions.queries?.retry).toBe(3);
  });

  it('should have staleTime set to 30000ms', () => {
    const defaultOptions = queryClient.getDefaultOptions();
    expect(defaultOptions.queries?.staleTime).toBe(30_000);
  });

  it('should have exponential backoff retryDelay', () => {
    const defaultOptions = queryClient.getDefaultOptions();
    const retryDelay = defaultOptions.queries?.retryDelay;
    if (typeof retryDelay === 'function') {
      expect(retryDelay(0, new Error())).toBe(1000);  // 1st retry: 1s
      expect(retryDelay(1, new Error())).toBe(2000);  // 2nd retry: 2s
      expect(retryDelay(2, new Error())).toBe(4000);  // 3rd retry: 4s
    }
  });
});
```

---

## 성능 기준 (Performance Criteria)

| 기준 | 목표값 | 측정 방법 |
|------|--------|-----------|
| 앱 시작 시간 (정상) | 현재 대비 +10% 이내 | 시작 ~ 메인 윈도우 표시까지 |
| Mutex 복구 오버헤드 | 5ms 이하/회 | 단위 테스트 타이밍 |
| 워커 스레드 종료 시간 | 5초 이내 | 종료 이벤트 ~ 스레드 완료 |
| 타임아웃 정확도 | 설정값 ±100ms | 타임아웃 발동 시점 측정 |
| 메모리 증가 없음 | Mutex 복구 후 메모리 누수 없음 | Valgrind 또는 heaptrack |

---

## 회귀 방지 기준

다음 동작들은 이번 수정 후에도 반드시 유지되어야 한다:

- [ ] Tauri IPC 명령 시그니처 변경 없음 (`src-tauri/src/lib.rs`의 `#[tauri::command]` 함수들)
- [ ] 큐 상태 JSON 직렬화 형식 변경 없음 (`queue_state.json` 구조)
- [ ] 설정 파일 JSON 형식 변경 없음 (`settings.json` 구조)
- [ ] 기존 정상 다운로드 플로우 동작 유지
- [ ] 기존 Tauri 이벤트 이름 및 페이로드 구조 변경 없음

---

## 수동 검증 시나리오 (E2E)

### 시나리오 E2E-001: 전체 다운로드 사이클

1. 앱을 시작한다
2. 유효한 YouTube URL을 입력하고 다운로드를 시작한다
3. 다운로드 진행 중에 취소 버튼을 클릭한다
4. 다운로드가 중단되고 프로세스가 정리되는지 확인한다 (작업 관리자/Activity Monitor)
5. 새 다운로드 작업을 시작한다 (이전 취소 후 정상 재시작 확인)

### 시나리오 E2E-002: 앱 강제 재시작 후 상태 복구

1. 다운로드 진행 중에 앱을 강제 종료한다
2. 앱을 재시작한다
3. 큐 상태가 손상 없이 로드되는지 확인한다
4. 이전 다운로드가 "interrupted" 상태로 표시되는지 확인한다

### 시나리오 E2E-003: 네트워크 불안정 환경

1. 네트워크 연결을 제한적으로 설정한다 (패킷 손실 시뮬레이션)
2. 다운로드를 시작한다
3. React Query가 자동으로 재시도하는지 UI에서 확인한다
4. 네트워크가 복구되면 정상적으로 재개되는지 확인한다
