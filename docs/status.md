# 진행 상황

2026-10-19 기준. 계획은 [report.md](report.md), 근거는 [research.md](research.md). 상태는 이 문서에만 적는다.

## 지금 단계

1단계 2번(터널 개념 검증) 마무리 중이다. 터널, `media://` 이미지, 연결 상태와 킬 스위치는 됐다. 같은 날 코드·문서를 재검토해 나온 문제는 모두 고쳤다(아래). 남은 것은 동영상 재생(블록 캐시)이다.

## 구현 현황

| 영역 | 파일 | 상태 |
|---|---|---|
| 창·UI 껍데기 | `lib.rs`, `App.svelte`, `components/` | 반투명 사이드바, 떠 있는 본문 패널(반경 14px), 검색창(⌘K / Ctrl K, `/`), 빈 검색 화면과 예시 태그, 영어·한국어. 앱 아이콘(`src-tauri/icons/source.svg`가 원본) |
| 연결 표시 | `Sidebar.svelte`, `connection.svelte.ts` | 직접 / 연결 중 / `VPN · 서버` / 실패(원인은 마우스를 올리면) |
| 키체인 | `secrets.rs` | 완료 |
| `.conf` 파서 | `tunnel/config.rs` | 완료. 가져오기 화면은 1단계 9번 |
| Mullvad | `tunnel/mullvad.rs` | 토큰, 기기 등록·삭제, 서버 목록. 위치는 `jp-tyo` 고정 |
| 터널 | `tunnel/` | gotatun + smoltcp + SOCKS5. 핸드셰이크가 끝나야 시작 성공, 무응답 연결은 45초에 끊음 |
| 킬 스위치 | `network.rs`, `tunnel/socks.rs` | report.md 6.3절 1~3 구현. 4·5(실행 중 전환·끊김 감지)는 1단계 9번 |
| 실패 코드 | `network.rs` → `connection.svelte.ts` → i18n | report.md 6.4절의 6가지 |
| 타임아웃 | `network.rs`, `tunnel/` | report.md 6.5절 |
| `media://` | `media.rs`, `media.ts` | 이미지 정상 표시. 동영상은 블록 캐시가 필요(report.md 7.2절) |
| 라이선스 | `LICENSE` | MIT. 배포물 고지는 1단계 11번 |
| API 계층 이후 (1단계 3~11번) | | 시작 전 |

## 재검토에서 고친 것 (2026-10-19)

| # | 문제 | 고친 방법 | 확인 |
|---|---|---|---|
| 1 | 키체인 읽기 오류를 "기기 없음"으로 취급해 직접 연결로 시작함(킬 스위치가 열림) | 막힌 채 시작, 키체인은 블로킹 스레드에서 한 번 읽고 "없음"이 확인될 때만 직접 연결 | 고치기 전 재현 테스트: 키체인 오류 뒤 상태 `Direct`, 요청이 서버에 닿음. 고친 뒤 `network.rs` 테스트 4개 |
| 2 | 핸드셰이크 전에 `tunnel`을 알림 | `force_handshake`가 10초 안에 끝나야 시작 성공, 아니면 `handshake` 실패 | 실제 앱: 핸드셰이크 뒤 `tunnel up via jp-tyo-wg-001`, 사이드바 `VPN · jp-tyo-wg-001` |
| 3 | 터널이 끊기면 요청이 끝나지 않음 | smoltcp keep-alive 15초 + 무응답 45초 timeout, reqwest 연결·읽기 30초, Mullvad 요청 30초 | 임시 점검 프로그램(상대 stack을 붙였다 끊음): 고치기 전 75초 넘게 대기, 고친 뒤 45.0초에 끝남 |
| 4 | 본문 패널 왼쪽 모서리가 4px | 반경 토큰을 `sm`/`md`/`lg`로, 기본 반경 단계 제거 | WKWebView 계산값 14/14/14/14px, 실제 앱 캡처 |
| 5 | 사이드바 연결 표시가 "Direct connection" 고정 | 연결 상태를 읽어 표시 | 실제 앱 캡처 |
| 6 | smoltcp 난수 시드 0 | OS 난수로 채움 | 테스트: 시드 0이면 두 스택의 첫 SYN 순서 번호가 같음(-34801502), 고친 뒤 다름 |
| 7 | `wg.rs`, `socks.rs` 주석이 사실과 다름 | 수정, `set_route`에 적용 범위 명시 | |
| 8 | 연결 상태 첫 값 경합 | 이벤트가 먼저 왔으면 명령 응답을 버림 | 타입 검사만(경합 재현은 못 함) |
| 9 | 실패 사유를 영어 문장으로 화면에 보냄 | 실패 코드 + 언어별 문구 | |
| 10 | `cap_range` 덧셈 넘침 | 포화 덧셈 | 테스트: 고치기 전 패닉, 고친 뒤 통과 |
| 11 | ⌘K가 macOS에서 Ctrl K도 가로채고, Caps Lock·한글 입력에서 안 먹을 수 있음 | 플랫폼별 수식키, 물리 키(`KeyK`)로 판별 | 타입 검사만. 키 입력 자동화 권한이 없어 실제 입력은 못 해 봄 |
| 12 | 빌드 경고, clippy 경고 2건, rustfmt 미적용, 안 쓰는 `ipnetwork`, `tunnel_probe`의 UA 중복, `index.html`의 남은 `no-referrer` | 정리 | 경고 0 |
| 13 | `LICENSE` 없음 | MIT 추가 | |

## 검증 기록

| 날짜 | 무엇 | 방법 | 결과 |
|---|---|---|---|
| 10-19 | 앱 터널로 e621 접속 | `tunnel_probe`, 시스템 VPN 끔(e621 직접 접속은 차단됨) | Mullvad 출구 확인, e621 API 200 (364ms) |
| 10-19 | 터널 처리량 | Cloudflare 10MB | 터널 5.2 MB/s, 직접 9.4 MB/s(경로가 달라 같은 조건 아님) |
| 10-19 | `media://` 이미지 | 실제 앱 | 정상 표시 |
| 10-19 | `media://` 동영상 | 실제 앱, 디버그 로그 | 재생 안 됨. 작은 Range 수백 번(research.md 7절) |
| 10-19 | 코드·문서 재검토 | 의존성 소스 대조, WKWebView 계산값·언어 실측 | 위 표. 확인한 동작은 research.md 8절 |
| 10-19 | 재검토 수정 후 | `cargo test`(14), `cargo clippy --all-targets`, `cargo fmt --check`, `npm run check`, `npm run build`, 실제 앱 실행·캡처 | 모두 통과, 경고 0. 앱은 한국어로 뜨고 터널 연결 후 사이드바에 서버 표시 |

## 남은 일

- **동영상 블록 캐시**(report.md 7.2절). 먼저 720p 샘플 처리량을 잰다.
- 동영상 재생·탐색 반응성 확인. 확인용 임시 패널은 커밋하지 않는다. `canLoadRemote()`가 참일 때만 `mediaUrl(...)`로 `<img>`/`<video>`를 띄우면 된다(예: 게시물 6791442의 720p, 6791519의 원본 PNG).
- 핸드셰이크까지 포함한 터널 시작 시간을 `tunnel_probe`로 다시 잰다(research.md 7절의 "수 ms"는 핸드셰이크 전 수치).
- 터널 안 연결이 리셋되거나 timeout으로 끊기면 읽기가 오류가 아니라 EOF로 끝난다. 지금은 모든 요청이 TLS라 잘림이 오류로 드러나지만, 평문 요청을 쓰게 되면 `netstack.rs`에서 구분해야 한다.
- 1단계 9번: 실행 중 끊김 감지·재연결, 라우트 전환 시 연결 끊기(report.md 6.3절 4·5), 서버 목록 디스크 캐시(터널 시작 3~4초의 대부분), 키체인 실패 후 다시 시도.
- Windows 실측 전부: 터널, aws-lc-rs 빌드, WebView2의 `media://` Range, Mica, Ctrl K. Windows 기기나 VM이 필요하다.
- macOS WKWebView의 WebM 재생 확인.
- Developer ID Application 인증서 발급(1단계 11번 전, 키체인에 아직 없음).
- 배포물의 의존성 라이선스 고지(1단계 11번).

## 다음 할 일

1. 핸드셰이크 포함 터널 시작 시간을 `tunnel_probe`로 잰다.
2. 720p 샘플 처리량을 재고 블록 크기를 정한다.
3. 동영상 블록 캐시, 재생·탐색 확인.
4. 1단계 3번 API 계층.

## 개발 메모

- 터널 수동 점검: `cd src-tauri && cargo run --example tunnel_probe` (`--forget`은 Mullvad 기기 등록 해제, `PROBE_DOWNLOAD=<url>`은 속도 측정). 출력의 "tunnel up in"은 이제 핸드셰이크까지 포함한다.
- Mullvad 계정 번호는 사용자가 직접 넣는다: `security add-generic-password -s pro.tb.nyabi.Tailgate -a mullvad-account -w`. 이 계정에는 지금 `divine condor` 기기가 등록되어 있다.
- 키체인 항목을 다른 실행 파일이 만들었거나 다시 빌드한 디버그 실행 파일이 읽으면 macOS가 접근 허용 창을 띄운다. 창이 떠 있는 동안 앱은 `connecting`으로 기다리고, 거부하면 `failed`(`credentialStore`)로 막힌 채 남는다.
- 디버그 로그: `RUST_LOG=info,tailgate_lib=debug npm run tauri dev`. 웹 인스펙터는 `TAILGATE_DEVTOOLS=1`(디버그 빌드에서만).
- 검사: `cargo test`, `cargo clippy --all-targets`, `cargo fmt --check`(`src-tauri`), `npm run check`, `npm run build`. Svelte 경고는 `npm run check`가 아니라 `npm run build`에서만 나오는 것이 있으니 둘 다 본다.
- 화면 확인: 키 입력 자동화(손쉬운 사용) 권한은 없고 화면 기록 권한은 있다. 창 캡처는 `CGWindowListCopyWindowInfo`로 창 번호를 찾아 `screencapture -l <번호>`. CSS 계산값은 작은 Swift 프로그램으로 WKWebView에 빌드된 CSS를 띄워 `getComputedStyle`로 잰다.
