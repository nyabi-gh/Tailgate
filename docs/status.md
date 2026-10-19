# 진행 상황

2026-10-19 기준. 계획은 [report.md](report.md), 근거는 [research.md](research.md). 상태는 이 문서에만 적는다.

## 지금 단계

1단계 2번(터널 개념 검증) 마무리 중이다. 터널, `media://` 이미지, 연결 상태는 됐다. 같은 날 코드·문서를 재검토해 나온 문제를 고치는 중이고, 그다음 동영상 블록 캐시로 간다.

## 구현 현황

| 영역 | 파일 | 상태 |
|---|---|---|
| 창·UI 껍데기 | `lib.rs`, `App.svelte`, `components/` | 반투명 사이드바, 떠 있는 본문 패널, 검색창, 빈 검색 화면과 예시 태그, 영어·한국어. 앱 아이콘(`src-tauri/icons/source.svg`가 원본) |
| 키체인 | `secrets.rs` | 완료 |
| `.conf` 파서 | `tunnel/config.rs` | 완료. 가져오기 화면은 1단계 9번 |
| Mullvad | `tunnel/mullvad.rs` | 토큰, 기기 등록·삭제, 서버 목록. 위치는 `jp-tyo` 고정 |
| 터널 | `tunnel/` | gotatun + smoltcp + SOCKS5. macOS에서 e621 API 200 |
| 연결 상태·킬 스위치 | `network.rs`, `connection.svelte.ts` | Rust → 화면 이벤트, 준비 전 원격 요청 차단 |
| `media://` | `media.rs`, `media.ts` | 이미지 정상 표시. 동영상은 블록 캐시가 필요(report.md 7.2절) |
| API 계층 이후 (1단계 3~11번) | | 시작 전 |

## 재검토에서 나온 문제 (2026-10-19, 고치는 중)

| # | 문제 | 고치는 방법 |
|---|---|---|
| 1 | 키체인 읽기 오류를 "기기 없음"으로 취급해 직접 연결로 시작함(킬 스위치가 열림) | 막힌 채 시작, 키체인을 한 번 읽어 확인된 경우에만 열기(report.md 6.3절 1) |
| 2 | 핸드셰이크 전에 `tunnel`을 알림. 서버가 답하지 않아도 `failed`가 되지 않음 | `force_handshake`를 10초 안에 끝내야 시작 성공(6.3절 2) |
| 3 | 터널이 끊기면 요청이 끝나지 않음(smoltcp timeout 없음, reqwest 타임아웃 없음) | 6.5절 타임아웃 |
| 4 | 본문 패널 왼쪽 모서리가 4px(반경 토큰 이름이 Tailwind 내장과 겹침) | 반경 토큰을 sm/md/lg로(8절) |
| 5 | 사이드바 연결 표시가 "Direct connection" 고정 | 연결 상태를 읽어 표시(8절) |
| 6 | smoltcp 난수 시드 0 | OS 난수로 채움(6.6절) |
| 7 | `wg.rs` 주석의 근거가 사실과 다름, `socks.rs` 주석이 웹뷰도 SOCKS를 쓴다고 함 | 주석 수정 |
| 8 | 연결 상태 첫 값 경합(늦은 명령 응답이 새 이벤트를 덮음) | 이벤트가 먼저 왔으면 응답을 버림(6.4절) |
| 9 | 실패 사유를 영어 문장으로 화면에 보냄 | 실패 코드(6.4절) |
| 10 | `cap_range` 덧셈 넘침 | 포화 덧셈 |
| 11 | ⌘K가 macOS에서 Ctrl K도 가로채고, Caps Lock·한글 입력에서 안 먹을 수 있음 | 플랫폼별 수식키, 물리 키로 판별(8절) |
| 12 | 빌드 경고(Svelte `state_referenced_locally`), clippy 경고 2건, rustfmt 미적용, 안 쓰는 `ipnetwork` 의존성, `tunnel_probe`의 UA 중복, `index.html`의 남은 `no-referrer` | 정리 |
| 13 | `LICENSE` 없음 | MIT `LICENSE` 추가 |

실행 중 라우트 전환 시 연결 끊기와 끊김 감지는 지금 코드에서 문제가 되지 않아 1단계 9번에서 한다(report.md 6.3절 4·5).

## 검증 기록

| 날짜 | 무엇 | 방법 | 결과 |
|---|---|---|---|
| 10-19 | 앱 터널로 e621 접속 | `tunnel_probe`, 시스템 VPN 끔(e621 직접 접속은 차단됨) | Mullvad 출구 확인, e621 API 200 (364ms) |
| 10-19 | 터널 처리량 | Cloudflare 10MB | 터널 5.2 MB/s, 직접 9.4 MB/s(경로가 달라 같은 조건 아님) |
| 10-19 | `media://` 이미지 | 실제 앱 | 정상 표시 |
| 10-19 | `media://` 동영상 | 실제 앱, 디버그 로그 | 재생 안 됨. 작은 Range 수백 번(research.md 7절) |
| 10-19 | 코드·문서 재검토 | 의존성 소스 대조, WKWebView 계산값·언어 실측 | 위 표. 확인한 동작은 research.md 8절 |

## 남은 일

- **동영상 블록 캐시**(report.md 7.2절). 먼저 720p 샘플 처리량을 잰다.
- 동영상 재생·탐색 반응성 확인. 확인용 임시 패널은 커밋하지 않는다. `canLoadRemote()`가 참일 때만 `mediaUrl(...)`로 `<img>`/`<video>`를 띄우면 된다(예: 게시물 6791442의 720p, 6791519의 원본 PNG).
- 1단계 9번: 실행 중 끊김 감지·재연결, 라우트 전환 시 연결 끊기, 서버 목록 디스크 캐시(터널 시작 3~4초의 대부분), 키체인 실패 후 다시 시도.
- Windows 실측 전부: 터널, aws-lc-rs 빌드, WebView2의 `media://` Range, Mica. Windows 기기나 VM이 필요하다.
- macOS WKWebView의 WebM 재생 확인.
- Developer ID Application 인증서 발급(1단계 11번 전, 키체인에 아직 없음).
- 배포물의 의존성 라이선스 고지(1단계 11번).

## 다음 할 일

1. 재검토 문제 고치기(위 표).
2. 핸드셰이크까지 포함한 터널 시작 시간을 `tunnel_probe`로 다시 잰다.
3. 720p 샘플 처리량을 재고 블록 크기를 정한다.
4. 동영상 블록 캐시, 재생·탐색 확인.
5. 1단계 3번 API 계층.

## 개발 메모

- 터널 수동 점검: `cd src-tauri && cargo run --example tunnel_probe` (`--forget`은 Mullvad 기기 등록 해제, `PROBE_DOWNLOAD=<url>`은 속도 측정).
- Mullvad 계정 번호는 사용자가 직접 넣는다: `security add-generic-password -s pro.tb.nyabi.Tailgate -a mullvad-account -w`. 이 계정에는 지금 `divine condor` 기기가 등록되어 있다.
- 키체인 항목을 다른 실행 파일이 만들었거나 다시 빌드한 디버그 실행 파일이 읽으면 macOS가 접근 허용 창을 띄운다. 창이 떠 있는 동안 앱은 `connecting`으로 기다리고, 거부하면 `failed`(`credentialStore`)로 막힌 채 남는다.
- 디버그 로그: `RUST_LOG=info,tailgate_lib=debug npm run tauri dev`. 웹 인스펙터는 `TAILGATE_DEVTOOLS=1`(디버그 빌드에서만).
- 검사: `cargo test`, `cargo clippy --all-targets`, `cargo fmt --check`(`src-tauri`), `npm run check`, `npm run build`.
- 화면 캡처 검증은 터미널에 화면 기록 권한이 있어야 한다. 키 입력 자동화(손쉬운 사용) 권한은 없다.
