---
id: SPEC-STABILITY-001
document: plan
version: 1.0.0
---

# SPEC-STABILITY-001 구현 계획

<!-- TAG: SPEC-STABILITY-001 -->

## 개요

본 계획은 TubeExtract Rust 백엔드의 4가지 치명적 안정성 이슈와 2가지 높은 우선순위 이슈를 해결하기 위한 구현 접근 방식을 정의한다. 개발 방법론은 **DDD (ANALYZE-PRESERVE-IMPROVE)** 를 따른다.

---

## 마일스톤

### Primary Goal: Critical 이슈 해결 (REQ-001 ~ REQ-004)

앱 즉시 충돌을 유발하는 4가지 이슈를 가장 먼저 처리한다.

#### M1-A: Mutex Poisoning 패닉 제거 (REQ-001)
- **수정 파일**: `src-tauri/src/lib.rs`
- **대상 라인**: 1034, 1155, 1205, 1248, 1310, 1340, 1372, 1877 (총 8곳)
- **의존성**: 없음 (독립 수정)

**기술적 접근:**

DDD ANALYZE 단계에서 8개 발생 지점의 컨텍스트를 각각 확인한다:
- 각 잠금 후 어떤 데이터를 읽거나 수정하는지 파악
- 독점된 데이터가 손상 가능한지 평가

DDD PRESERVE 단계에서 characterization test 작성:
- 현재 Mutex 잠금 성공 경로의 동작을 캡처
- 잠금 실패 시 패닉이 발생하는 현재 동작을 테스트로 문서화

DDD IMPROVE 단계에서 수정 적용:
```rust
// 변경 전
let mut state = shared.lock().unwrap_or_else(|_| panic!("state lock poisoned"));

// 변경 후
let mut state = shared.lock().unwrap_or_else(|e| {
    log::error!("[STABILITY] Mutex poisoned at {}, recovering: {:?}",
                std::panic::Location::caller(), e);
    e.into_inner()
});
```

**주의 사항:**
- `into_inner()` 복구 후 상태 데이터의 유효성을 검사하는 로직 추가 고려
- 복구된 상태가 불일치 상태일 경우 해당 작업만 실패 처리 (앱 전체 충돌 방지)

---

#### M1-B: Tauri 시작 패닉 제거 (REQ-004)
- **수정 파일**: `src-tauri/src/lib.rs`
- **대상 라인**: 2061
- **의존성**: 없음 (독립 수정)

**기술적 접근:**

```rust
// 변경 전
.run(tauri::generate_context!())
.expect("error while running tauri application");

// 변경 후
if let Err(e) = builder.run(tauri::generate_context!()) {
    // 1. 오류 로그 파일에 기록
    eprintln!("[FATAL] Tauri initialization failed: {:?}", e);

    // 2. 사용자 친화적 메시지 표시 시도
    // Tauri가 아직 초기화되지 않았으므로 네이티브 다이얼로그 사용
    // (플랫폼별 처리 필요)

    // 3. 정리된 종료
    std::process::exit(1);
}
```

**플랫폼별 처리:**
- Windows: `MessageBoxW` WinAPI 또는 `native-dialog` 크레이트 검토
- macOS: `NSAlert` 또는 stderr 출력 후 프로세스 종료
- Linux: stderr 출력 후 프로세스 종료

---

#### M1-C: 명령 실행 타임아웃 구현 (REQ-003)
- **수정 파일**: `src-tauri/src/lib.rs`
- **대상 함수**: `run_command_capture` (lines 402-436)
- **의존성**: 없음 (독립 수정)

**기술적 접근:**

```rust
// 파라미터 이름 수정 (_timeout_ms -> timeout_ms)
fn run_command_capture(
    app: &AppHandle,
    command: &str,
    args: &[&str],
    timeout_ms: u64,  // 더 이상 사용하지 않는 변수 경고 제거
) -> CommandCaptureResult {
    // ...

    if timeout_ms > 0 {
        // 방법 A: wait_timeout 사용 (권장)
        match child.wait_timeout(Duration::from_millis(timeout_ms)) {
            Ok(Some(status)) => { /* 정상 완료 */ }
            Ok(None) => {
                // 타임아웃 - 프로세스 강제 종료
                let _ = child.kill();
                return CommandCaptureResult::Timeout;
            }
            Err(e) => { /* 오류 처리 */ }
        }
    } else {
        // timeout_ms == 0: 무제한 대기 (기존 동작 유지)
        let _ = child.wait();
    }
}
```

**참고:** `wait_timeout`은 `std::process::Child`의 표준 라이브러리 메서드가 아니므로, 별도 watchdog 스레드 패턴을 사용할 수 있다:
```rust
// Watchdog 스레드 패턴 (대안)
let (tx, rx) = std::sync::mpsc::channel();
let child_id = child.id();
std::thread::spawn(move || {
    std::thread::sleep(Duration::from_millis(timeout_ms));
    tx.send(()).ok();
});
// select 또는 polling으로 종료/타임아웃 감지
```

---

#### M1-D: 워커 스레드 종료 채널 (REQ-002)
- **수정 파일**: `src-tauri/src/lib.rs`
- **대상 라인**: 1170-1388 (워커 루프), 1287-1304 (stdout/stderr 스레드)
- **의존성**: M1-A (Mutex 복구) 완료 후 진행 권장

**기술적 접근:**

1. 앱 State에 shutdown channel sender 추가:
```rust
// AppState 또는 SharedState에 추가
struct WorkerState {
    // 기존 필드들...
    shutdown_tx: Option<std::sync::mpsc::Sender<()>>,
    worker_handle: Option<std::thread::JoinHandle<()>>,
}
```

2. 워커 루프에 종료 신호 폴링 추가:
```rust
loop {
    // 종료 신호 체크 (non-blocking)
    if shutdown_rx.try_recv().is_ok() {
        log::info!("[WORKER] Shutdown signal received, exiting loop");
        break;
    }

    // 기존 큐 처리 로직...
}
```

3. 앱 종료 훅에서 종료 신호 전송:
```rust
// on_window_close_requested 또는 on_exit 핸들러에서
if let Some(tx) = state.shutdown_tx.take() {
    let _ = tx.send(());
}
if let Some(handle) = state.worker_handle.take() {
    // 최대 5초 대기
    // handle.join()은 blocking이므로 별도 스레드에서 처리
}
```

---

### Secondary Goal: High Priority 이슈 해결 (REQ-005, REQ-006)

#### M2-A: 프로세스 종료 검증 (REQ-005)
- **수정 파일**: `src-tauri/src/lib.rs`
- **대상 함수**: `terminate_child_with_grace_period` (lines 1124-1138)
- **의존성**: 없음 (독립 수정)

**기술적 접근:**
- 500ms grace period 후 `child.try_wait()` 로 종료 여부 확인
- Windows에서는 `std::process::Command::new("taskkill")` 를 통해 `/F /T /PID {pid}` 실행으로 자식 프로세스 트리 종료

---

#### M2-B: React Query 설정 업데이트 (REQ-006)
- **수정 파일**: `src/renderer/lib/queryClient.ts`
- **대상 라인**: 3-10
- **의존성**: 없음 (독립 수정)

**기술적 접근:**
```typescript
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

### Final Goal: 테스트 및 검증

#### M3-A: Rust 단위 테스트 작성
- Mutex 복구 경로 테스트
- 타임아웃 발생 시나리오 테스트
- 워커 스레드 종료 시나리오 테스트

#### M3-B: 통합 테스트
- 앱 시작/종료 정상 사이클 검증
- 다운로드 취소 후 프로세스 정리 검증

---

## 파일 변경 목록

| 파일 | 변경 유형 | 관련 요구사항 |
|------|-----------|---------------|
| `src-tauri/src/lib.rs` | 수정 | REQ-001, REQ-002, REQ-003, REQ-004, REQ-005 |
| `src/renderer/lib/queryClient.ts` | 수정 | REQ-006 |
| `src-tauri/src/lib_test.rs` (신규) | 생성 | 테스트 커버리지 |

---

## 의존성 그래프

```
M1-A (Mutex 복구) ──┐
M1-B (시작 패닉)    ├──> M1-D (워커 스레드) ──> M3-A (테스트)
M1-C (타임아웃)  ──┘
                        M2-A (프로세스 종료)
M2-B (React Query) ──> 독립적 (언제든 적용 가능)
```

M1-A, M1-B, M1-C는 서로 독립적으로 병렬 작업 가능.
M1-D는 M1-A 완료 후 진행이 안전.
M2-A, M2-B는 M1 작업들과 독립적으로 진행 가능.

---

## 기술적 리스크

| 리스크 | 확률 | 영향 | 완화 전략 |
|--------|------|------|-----------|
| `into_inner()` 복구 후 상태 불일치로 인한 다운스트림 오류 | Medium | Medium | 복구 직후 상태 유효성 검사 추가 |
| `wait_timeout` 비표준 API 사용 불가 | Low | Low | Watchdog 스레드 패턴으로 대체 |
| Tauri 초기화 실패 시 UI 다이얼로그 표시 어려움 | Medium | Low | stderr 로그 + 로그 파일 기록으로 대체 |
| 워커 스레드 종료 시 큐 상태 손실 | Low | High | 종료 전 큐 영속화(persist) 확인 |
| Windows 자식 프로세스 종료 실패 | Medium | Medium | `taskkill /F /T` 명령 활용 |

---

## 테스트 전략 (DDD 모드)

### ANALYZE 단계
- `lib.rs`의 Mutex 잠금 패턴 8개 지점 모두 코드 리뷰
- `run_command_capture` 함수의 호출 지점과 타임아웃 값 확인
- 워커 스레드 루프의 현재 종료 조건 분석

### PRESERVE 단계 (Characterization Tests)
- 정상 다운로드 플로우 characterization test
- 큐 상태 직렬화/역직렬화 characterization test
- Tauri 명령 핸들러 응답 형식 characterization test

### IMPROVE 단계 (새 테스트)
- Mutex poisoning 복구 경로 단위 테스트
- 타임아웃 강제 종료 테스트 (mock process 사용)
- 워커 스레드 정상 종료 테스트 (mpsc channel)
- React Query retry 동작 확인 테스트

---

## 완료 기준 (Definition of Done)

- [ ] 8개의 `panic!("state lock poisoned")` 호출이 모두 `into_inner()` 복구 패턴으로 교체됨
- [ ] `run_command_capture`의 `_timeout_ms` 파라미터가 실제 타임아웃을 강제하도록 구현됨
- [ ] 워커 스레드가 mpsc 채널을 통해 정상 종료될 수 있음
- [ ] Tauri 시작 실패 시 `expect()` 대신 오류 처리 코드가 실행됨
- [ ] `terminate_child_with_grace_period`에서 실제 종료 검증이 이루어짐
- [ ] `queryClient.ts`에 retry: 3, staleTime: 30000 설정이 적용됨
- [ ] 모든 변경에 대한 characterization test가 작성됨
- [ ] Rust 컴파일 경고 0개 (unused variable `_timeout_ms` 포함)
- [ ] 기존 Tauri IPC 명령 시그니처 변경 없음 확인
