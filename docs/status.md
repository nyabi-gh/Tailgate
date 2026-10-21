# 진행 상황

2026-10-19 기준. 계획은 [report.md](report.md), 근거는 [research.md](research.md). 상태는 이 문서에만 적는다.

## 지금 단계

1단계 2번(터널 개념 검증)이 끝났다. 터널, `media://` 이미지와 동영상(블록 캐시), 연결 상태와 킬 스위치가 실제 앱에서 동작한다. 1단계 3번 API 계층(클라이언트·스로틀·모델·검색 페이지 넘김)과 4번 계정(로그인 시트, 키체인 저장, 시작 시 복원)도 됐다. 5번 검색 화면(태그 칩·자동완성·정렬, 3:4 격자, 가상·무한 스크롤, 크기 조절)도 실제 앱에서 동작한다. 다음은 6번 블랙리스트다.

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
| `media://` | `media/`, `media.ts` | 이미지는 통째로, 동영상은 1MB 블록 캐시(`media/blocks.rs`, 메모리 256MB, 다음 블록 미리 받기)로 응답. report.md 7.2절 |
| 라이선스 | `LICENSE` | MIT. 배포물 고지는 1단계 11번 |
| API 계층 | `api/`, `examples/api_probe.rs` | posts 검색(`PostSearch`, 100개씩, `b<id>` 또는 페이지 번호, 중복 제거), 단일 게시물, `users/me`, 자동완성. 요청 시작 간격 1초, UA, Basic 인증(자격 증명 설정 자리만, 저장은 4번), 429·503 재시도, 오류 코드. 화면·명령은 아직 없음 |
| 계정 | `account.rs`, `account.svelte.ts`, `SignInSheet.svelte` | 로그인 시트(사이드바), e621 확인 뒤 키체인 `e621-account`에 저장, 로그아웃, 시작 시 복원하고 연결이 열리면 `/users/me`로 확인, 401이면 저장된 키를 지우고 "다시 로그인". UA는 API와 `media://` 모두 로그인한 이름. API 키 발급 페이지는 `tauri-plugin-opener`(e621 주소만 허용) |
| 검색 | `search.rs`, `search.svelte.ts`, `layout.ts`, `tags.ts`, `TagInput`, `SortMenu`, `Grid`, `Thumb` | 태그 칩(카테고리 색: 자동완성과 첫 결과에서 앎, 메타태그는 meta 색, 제외는 취소선), 자동완성(3글자, 300ms, 접두어 캐시), 정렬 메뉴(`order:` 바꿈), 3:4 같은 크기 칸 격자(틈 3px, 그림은 칸을 채우게 자름. 2026-10-21 justified에서 바꿈), 가상 스크롤(±1화면), 남은 1.5화면에서 다음 페이지, ⌘/Ctrl+휠·슬라이더로 칸 폭 100~360(기본 180, 240 이상이면 `sample`), 로딩·빈 결과·오류·다시 시도, 계정이 바뀌면 다시 검색. 썸네일 클릭(뷰어)은 7번 |
| 블랙리스트 이후 (1단계 6~11번) | | 시작 전 |

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
| 10-19 | 핸드셰이크 포함 터널 시작 | `tunnel_probe` 37회(릴리스 34, 디버그 3) | 31~38ms가 35회, 99ms 1회, 5.3초 1회(원인 미확인). research.md 7절 |
| 10-19 | static1 처리량, 720p 샘플 | 같은 파일을 직접·터널로 번갈아, Range 크기별 | 직접 0.9~1.75, 터널 0.63~1.08 MB/s. 720p 비트레이트 0.11~0.14 MB/s. 블록 1MB로 결정(report.md 7.2절) |
| 10-19 | 블록 캐시 | `cargo test`(21, 가짜 static1 서버로 작은 Range·미리 받기·마지막 블록·범위 밖·Range 없음), 실제 앱(빈 캐시, 터널) | WKWebView의 8~24바이트 Range 수백 개가 블록 하나당 상위 요청 한 번으로 처리됨. 재생 시작 720p 샘플(1.6MB) 2.6초, 3분 MP4(21.9MB) 3.05초. 100초·30초 탐색 후 재생 확인, 사용자도 재생 확인. 탐색 시간은 창이 가려져 두 영상이 함께 멈춘 구간이 섞여 정확히 못 잼(블록 하나 받기 약 2초) |
| 10-19 | API 계층 | `cargo test`(39): Pawru 픽스처 디코딩(이미지·GIF·WebM·삭제·익명 me·자동완성), 가짜 서버로 UA·Authorization·재시도·오류 코드·페이지 넘김, 멈춘 시계로 시작 간격 | 통과 |
| 10-19 | API 실호출 | `api_probe`, 터널(`jp-tyo-wg-001`) | 익명 me, `wolf` 2페이지(커서 이어짐), `order:score`, `type:webm`의 720p·MP4, `status:deleted` 100개 모두 파일 URL 없음, 단일 게시물, 자동완성 모두 디코딩됨. 3일 전 픽스처와 구조 같음 |
| 10-19 | 계정 | `cargo test`(48): mock 키체인 + 가짜 e621로 로그인(이름 표기·저장·UA·Authorization), 401, 저장 실패, 로그아웃, 시작 시 복원·확인, 연결 전 대기, 사용 중 401, 저장값 손상, 키체인 오류. 실제 앱: 사용자가 직접 로그인 | 사이드바에 `Nyabi`, 키체인 항목 생성. 재시작하면 터널 연결 0.4초 뒤 `signed in to e621 as Nyabi (1138150)`. 로그아웃은 실제 앱에서 안 해 봄(테스트만) |
| 10-21 | 검색 | `cargo test`(52: 검색 명령 상태, 결과로 태그 카테고리 찾기), `npm test`(배치 7), 실제 앱(임시 코드로 `wolf` 검색 → 스크롤 → 크기 320, 터널) | 그리드·칩 색·정렬 메뉴·슬라이더 표시, 스크롤로 두 번째 페이지 받음(미리보기 요청 172건), 크기 320에서 `sample` 요청 33건 |
| 10-21 | 3:4 격자로 바꾼 뒤 | `npm test`(격자 9), 실제 앱(임시 코드로 `wolf` 검색 → 스크롤 → 칸 폭 260) | 기본 크기에서 한 줄 6칸, 260에서 4칸으로 바뀌고 보던 위치 유지, 칸 크기 모두 같음, 동영상 배지 표시 |
| 10-19 | 터널 직후 첫 요청 | 위 37회 | 2회 SOCKS `TtlExpired`(DNS 또는 연결 시간 초과). 원인 미확인 |

## 남은 일

- 자동완성 결과가 하나도 없을 때의 응답 모양은 아직 못 봤다(`zzzzqqqq`도 비슷한 태그가 와서 빈 결과가 안 나옴). 지금 코드는 배열만 받는다. `tags.json`은 빈 결과가 `{"tags":[]}`라 다를 수 있다.
- 태그 입력·자동완성·정렬 메뉴·⌘K는 실제 앱에서 손으로 써 보지 못했다(키 입력 자동화 권한 없음). 앱 밖 브라우저에서는 명령이 없어 자동완성이 안 뜬다. 사용자 확인이 필요하다.
- 검색 화면의 다크 모드는 아직 못 봤다(시스템이 라이트 모드).
- 썸네일 해상도 기준(칸 폭 240px)은 실제로 재 보지 않았다. 한 페이지 썸네일이 다 뜨는 시간은 재지 못했다(이미지 완료 시점이 로그에 없음).
- 최근 검색어, 정렬·썸네일 크기 기억은 10번(설정 저장)에서 한다.
- 로그인 시트의 다크 모드와 오류 표시는 실제 앱에서 아직 못 봤다(시스템이 라이트 모드, 앱 밖 브라우저에서는 명령이 없어 오류를 띄울 수 없음). 오류 경로는 테스트로만 확인.
- 탐색 반응 시간을 창이 앞에 있는 상태로 다시 잰다(가려진 창에서는 WebKit이 미디어 로딩과 타이머를 멈춘다). 부족하면 report.md 7.2절의 첫 블록 축소·동시 받기를 검토한다. 확인용 임시 패널은 커밋하지 않는다. `canLoadRemote()`가 참일 때만 `mediaUrl(...)`로 `<video>`를 띄운다(예: 게시물 6791442의 720p 샘플, 6791860의 3분 MP4).
- **터널 직후 첫 요청이 가끔 `TtlExpired`로 실패**(37회 중 2회, research.md 7절). 재현될 때 DNS 질의·응답이 오갔는지 보려면 smoltcp `log` 기능이나 netstack 임시 로그가 필요하다(다시 빌드하면 키체인 허용 창이 뜬다). 앱에서 생기면 첫 화면 요청이 10~15초 뒤 실패하므로 1단계 9번 전에 다룬다.
- 터널 시작이 5.3초 걸린 1회(첫 핸드셰이크 재전송으로 보임)가 다시 나오는지 지켜본다.
- 터널 안 연결이 리셋되거나 timeout으로 끊기면 읽기가 오류가 아니라 EOF로 끝난다. 지금은 모든 요청이 TLS라 잘림이 오류로 드러나지만, 평문 요청을 쓰게 되면 `netstack.rs`에서 구분해야 한다.
- 1단계 9번: 실행 중 끊김 감지·재연결, 라우트 전환 시 연결 끊기(report.md 6.3절 4·5), 서버 목록 디스크 캐시(터널 시작 3~4초의 대부분), 키체인 실패 후 다시 시도.
- Windows 실측 전부: 터널, aws-lc-rs 빌드, WebView2의 `media://` Range, Mica, Ctrl K. Windows 기기나 VM이 필요하다.
- macOS WKWebView의 WebM 재생 확인.
- Developer ID Application 인증서 발급(1단계 11번 전, 키체인에 아직 없음).
- 배포물의 의존성 라이선스 고지(1단계 11번).

## 다음 할 일

1. 1단계 6번 블랙리스트: 파서·평가기 테스트 먼저, `PostSearch`에서 걸러 숨긴 수를 함께 보내고 "N개 숨김 보기".
2. 터널 직후 첫 요청 실패의 원인 추적(위 "남은 일").

## 개발 메모

- API 수동 점검: `cd src-tauri && cargo run --example api_probe -- [태그]`. 앱과 같은 경로(저장된 기기면 터널)로 me, 검색 2페이지, `order:score`, WebM, 삭제된 게시물, 단일 게시물, 자동완성을 부른다. e621 응답 구조가 바뀌면 여기서 디코딩 오류로 드러난다.
- 터널 수동 점검: `cd src-tauri && cargo run --example tunnel_probe` (`--forget`은 Mullvad 기기 등록 해제, `PROBE_DOWNLOAD=<url>`은 속도 측정, `--serve`는 SOCKS5를 열어 두어 `curl -x socks5h://<주소>`로 터널을 쓸 수 있다). 출력의 "tunnel up in"은 이제 핸드셰이크까지 포함한다. 디버그·릴리스 실행 파일은 각각 처음 키체인을 읽을 때 허용 창이 뜬다.
- Mullvad 계정 번호는 사용자가 직접 넣는다: `security add-generic-password -s pro.tb.nyabi.Tailgate -a mullvad-account -w`. 이 계정에는 지금 `divine condor` 기기가 등록되어 있다.
- 키체인 항목을 다른 실행 파일이 만들었거나 다시 빌드한 디버그 실행 파일이 읽으면 macOS가 접근 허용 창을 띄운다. 창이 떠 있는 동안 앱은 `connecting`으로 기다리고, 거부하면 `failed`(`credentialStore`)로 막힌 채 남는다.
- 디버그 로그: `RUST_LOG=info,tailgate_lib=debug npm run tauri dev`. 웹 인스펙터는 `TAILGATE_DEVTOOLS=1`(디버그 빌드에서만).
- 검사: `cargo test`, `cargo clippy --all-targets`, `cargo fmt --check`(`src-tauri`), `npm test`, `npm run check`, `npm run build`. Svelte 경고는 `npm run check`가 아니라 `npm run build`에서만 나오는 것이 있으니 둘 다 본다.
- 화면 확인: 키 입력 자동화(손쉬운 사용) 권한은 없고 화면 기록 권한은 있다. 창 캡처는 `CGWindowListCopyWindowInfo`로 창 번호를 찾아 `screencapture -l <번호>`. CSS 계산값은 작은 Swift 프로그램으로 WKWebView에 빌드된 CSS를 띄워 `getComputedStyle`로 잰다.
