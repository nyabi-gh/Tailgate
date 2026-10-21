# 조사 결과

2026-10-19 기준. 1~6절은 구현 전 조사, 7절은 구현하며 잰 실측, 8절은 재검토 때 의존성 소스와 실측으로 확인한 동작이다. 계획은 [report.md](report.md), 진행 상황은 [status.md](status.md).

"확인"은 1차 자료(레지스트리, 공식 문서, 소스, 실제 요청)에서 직접 본 내용이다. "미확인"은 알려져 있지만 원문이나 실측으로 확인하지 못한 내용이다. 나중에 사실이 아니게 된 내용은 지우지 않고 바뀐 내용을 함께 적는다.

## 0. 로컬 개발 환경 (확인)

- macOS 27.0.1, Xcode 27.1
- Rust 1.98.1 (rustup), Node 26.12, npm, bun
- 코드 서명 인증서는 `Apple Development` 하나뿐이다. 배포용 **`Developer ID Application` 인증서는 없다.**

## 1. e621 접속 상태 (확인)

- `e621.net`은 이 네트워크에서 **TLS ClientHello 직후 연결이 리셋된다.** DNS는 정상이고, 같은 IP에 다른 SNI로 접속하면 정상 응답한다. 즉 SNI 기반 차단이다.
- `static1.e621.net`(이미지·동영상 파일)은 **정상 접속된다.**
  - 썸네일 요청은 Referer가 없거나 `tauri://localhost/`여도 200을 준다. **단, 이 시험에는 브라우저의 `Sec-Fetch-*` 헤더가 빠져 있었다. 실제 웹뷰 요청은 403이다(7절).**
  - 동영상은 Range 요청에 206을 준다.
  - `cache-control: max-age=2678400`(31일)이다.
  - `access-control-allow-origin: https://e621.net`이라 웹뷰가 직접 받으면 JS `fetch`나 canvas로는 읽을 수 없다.
  - **이 세 줄은 웹뷰가 static1을 직접 받는다는 처음 계획을 전제로 했다.** 지금은 미디어가 `media://`를 거치므로 웹뷰 HTTP 캐시는 적용되지 않고(캐시는 앱이 갖는다, report.md 7.3절), 동영상 Range는 7절의 문제가 있다. CORS 헤더는 앱이 응답을 만들므로 필요하면 `media.rs`에서 붙일 수 있다.
- e621의 DNS HTTPS 레코드에 ECH 설정(공개 이름 `cloudflare-ech.com`)이 있다. 앱 자체에서 차단을 피하는 기술적 여지는 있지만, 이번 조사에서는 우회 접속을 시도하지 않았다.
- 결론: **API 동작은 Pawru의 조사 자료와 픽스처를 기준으로 한다.** 실제 API 호출 검증은 사용자가 쓰는 접속 수단(VPN 등)이 켜진 상태에서 해야 한다.

## 2. e621 API

Pawru `docs/research.md`(2026-10-16, 실호출 확인)를 그대로 따른다. 데스크톱 앱에 중요한 부분만 요약한다.

- **인증**: HTTP Basic `username:api_key`. 쿼리 방식은 쓰지 않는다. `/users/me.json`은 로그아웃 상태에서도 200이므로 `id`가 null인지로 판단한다.
- **User-Agent 필수**: `Tailgate/<버전> (by <username> on e621)`. 코드는 `Cargo.toml` 버전을 넣어 `Tailgate/0.1.0 (by Nyabi on e621)`을 보낸다. 브라우저 UA 흉내는 금지. 웹뷰가 API를 직접 부르면 브라우저 UA가 나가므로 **API는 반드시 Rust 쪽에서 호출한다.**
- **Rate limit**: 초당 2회 하드 리밋(503), 권장 초당 1회. 쓰기 요청은 토큰 버킷(429). 정적 파일은 대상 아님.
- **페이지네이션**: `page=b<id>` 커서는 깊이 제한 없음(단 `order:id_desc` 강제). `page=N`은 750까지, 넘으면 410.
- **미디어 필드**: `preview`(256px, `width`/`height` 포함), `sample`(최대 850px), `file`(원본). WebM 게시물은 `sample.alternates`에 H.264 MP4(`variants.mp4`, `samples.720p`/`480p`)가 있다.
- **숨김/삭제 게시물**: URL 필드가 null로 온다.
- **블랙리스트**: 사이트도 클라이언트(브라우저)에서 거른다. 앱이 직접 구현해야 한다. 아래는 2026-10-21 e621ng master의 `app/javascript/src/js/core/blacklist/`(`Filter.ts`, `FilterToken.ts`, `FilterUtilities.ts`)와 `models/PostCache.ts`를 읽고 확인한 동작이다. `blacklist.rs`가 그대로 옮겼다.
  - 한 줄이 필터 하나. 앞뒤 공백을 지우고 빈 줄과 `#`로 시작하는 줄은 버린다. 소문자로 바꾸고 ` #` 뒤를 잘라 낸다. 공백으로 나눈 토큰은 중복을 없앤다.
  - 보통 토큰은 모두 맞아야 하고(AND), `~` 토큰이 있으면 그중 하나가 맞아야 한다. `-`는 토큰 결과를 뒤집는다. 필터 중 하나라도 맞으면 숨긴다.
  - 메타태그: `tagcount`, `id`, `status`, `rating`, `type`, `width`, `height`, `filesize`, `score`, `favcount`, `fav`, `uploader`/`user`, `userid`, `username`, `pool`. 목록에 없는 `이름:값`은 그 문자열 그대로 태그다. `*`가 있는 태그는 와일드카드(`*`만 특수 문자).
  - 비교는 `<`, `<=`, `>`, `>=`(`=<`, `=>`도 받음)와 범위 `a..b`, `..b`, `a..`. 앞에 붙은 `=`는 값에 남아 숫자가 안 되므로 그 토큰은 늘 거짓이다. 숫자는 JavaScript `parseInt`처럼 앞쪽 숫자만 읽는다(`2abc`는 2).
  - `filesize`는 `5mb`, `300kb`, `1024b`를 받고, `=`는 ±5% 범위다. `rating`은 `safe`/`s` 등 긴 이름도 받는다. `fav:`는 값과 관계없이 "내 즐겨찾기". `user:!123`은 업로더 id. 업로더 이름은 소문자로 비교한다.
  - 게시물 쪽 값: 태그는 모든 카테고리를 합친 목록, 점수는 `score.total`, 크기는 원본 파일. `status`는 `pending`/`flagged`/`deleted` 깃발(사이트 템플릿이 주는 문자열은 확인하지 않고 이 셋으로 보았다).
  - 로그아웃 상태의 `/users/me`에도 사이트 기본 블랙리스트가 들어 있다(픽스처: gore, `young -rating:s` 등). 사이트는 익명 사용자에게 이것을 적용한다.
- **약관**: 개인적·비상업적 용도, 18세 이상. 대량 수집은 `db_export`를 쓰라고 안내한다. 일괄 다운로드 기능을 넣는다면 API 스로틀을 그대로 거쳐야 한다.

Pawru의 테스트 픽스처(`PawruTests/Fixtures/*.json`, 실제 응답)를 재사용할 수 있다.

2026-10-19 `api_probe`로 터널을 거쳐 다시 확인한 것:
- 응답 구조는 2026-10-16 픽스처와 같다(검색, 단일 게시물, 익명 `me`, 자동완성).
- `status:deleted` 검색 100개는 모두 파일 URL이 null이었다(익명).
- 자동완성은 철자가 비슷한 태그도 돌려준다. `zzzzqqqq`에 `zzz`, `zzz4` 등이 왔다.
- `page=b<id>`로 이어 받은 두 번째 페이지는 첫 페이지 최소 id보다 작은 id부터 시작했다(겹침 없음).

## 3. Rust 데스크톱 UI 선택지

버전은 crates.io/npm에서 직접 확인했다.

| 후보 | 최신 | 렌더링 | 이 앱 관점의 평가 |
|---|---|---|---|
| **Tauri** | 2.12.4 (2026-10-14) | 시스템 웹뷰 (macOS WKWebView, Windows WebView2) | GIF·WebM·MP4 재생, 이미지 디코딩·캐시, 스크롤 성능을 웹뷰가 해결한다. 번들러·서명·공증·업데이터가 내장. 3.0은 아직 alpha(CEF 런타임 실험 중). |
| Dioxus | 0.7.10 | 데스크톱은 역시 웹뷰(wry) | UI까지 Rust로 쓸 수 있다. 미디어 처리 이점은 Tauri와 같지만 생태계·도구가 얇고 0.x라 API가 계속 바뀐다. |
| egui | 0.37.0 | 즉시 모드, GPU | 도구·편집기용. **동영상 재생이 없어** ffmpeg/gstreamer를 붙여야 한다. 애니메이션 GIF, 대형 이미지 다운샘플링도 직접. 디자인 자유도가 낮다. |
| iced | 0.14.0 (2025-12) | 리테인드, GPU | 위와 같은 미디어 문제. 릴리스 간격이 길다. |
| Slint | 1.18.1 | 자체 마크업, GPU | 임베디드 강세. 동영상 재생 없음. |
| GPUI | 0.2.2 (2025-10) | Zed용 | 일반 앱용으로 문서·릴리스가 부족하다. |

**이미지 뷰어의 핵심 난제는 미디어**(애니메이션 GIF, WebM/MP4, 수천 장 썸네일 캐시)다. 네이티브 Rust GUI는 이걸 전부 직접 만들어야 하고, 동영상은 ffmpeg 같은 무거운 C 의존성과 배포 시 라이선스·서명 문제를 끌고 온다. **Tauri 2 + Rust 백엔드가 가장 현실적이다.**

### 웹뷰의 미디어 지원
- **WebView2 (Windows, Chromium 기반)**: WebM(VP8/VP9/AV1)과 H.264 MP4 모두 재생. (일반적으로 알려진 사실, 이번에 실측은 안 함)
- **WKWebView (macOS)**: Safari는 macOS 11.3부터 WebM/VP9를 재생한다. 다만 **서드파티 앱의 WKWebView에서 VP9가 기본으로 켜져 있는지는 미확인**이다(VideoToolbox 보조 디코더 등록이 필요하다는 포럼 보고가 있음). → MP4 변형을 우선 쓰면 문제가 되지 않는다. 1단계에서 실측한다.
- 애니메이션 GIF는 두 웹뷰 모두 `<img>`로 재생한다.

### 프런트엔드 프레임워크
| 후보 | 최신 | 비고 |
|---|---|---|
| **Svelte 5** | 5.57.2 (2026-10-06) | 런타임이 작고 반응성(runes)이 단순하다. 가상 스크롤 같은 성능 민감 코드를 직접 짜기 쉽다. |
| React | 19.3.1 | 생태계는 가장 크지만 이 규모에서는 이점이 적다. |
| SolidJS | 1.9.17 | 성능은 좋지만 생태계가 작다. |

- Vite 8.3.4, `@sveltejs/vite-plugin-svelte` 7.3.1
- **TypeScript 7.0.2(Go 네이티브 컴파일러)가 최신이다.** 하지만 `svelte-check` 4.7.7의 peer 범위가 `^5 || ^6`이라 **6.0.3에 고정했다**(확인).
- Tailwind CSS 4.3.3 + `@tailwindcss/vite` 4.3.3, `@lucide/svelte` 1.59.0 (설치 시 확인).

## 4. Rust 쪽 주요 크레이트 (버전 확인)

| 용도 | 크레이트 | 버전 |
|---|---|---|
| 앱 | tauri / tauri-build | 2.12.4 / 2.7.3 |
| HTTP | reqwest (기본 TLS가 rustls) | 0.13.5 |
| 비동기 | tokio | 1.53.3 |
| 직렬화 | serde | 1.0.229 |
| 오류 | thiserror | 2.0.21 |
| 자격 증명 | keyring-core + apple-native-keyring-store / windows-native-keyring-store | 1.0.1 / 1.0.2 / 1.1.0 |
| 창 상태 기억 | tauri-plugin-window-state | 2.5.0 |
| 중복 실행 방지 | tauri-plugin-single-instance | 2.5.4 |
| 파일 저장 대화상자 | tauri-plugin-dialog | 2.8.3 |
| 외부 링크 열기 | tauri-plugin-opener | 2.7.2 |
| 자동 업데이트 | tauri-plugin-updater | 2.14.0 |

- `keyring` 4.x는 `keyring-core` + 플랫폼별 저장소 크레이트 구조로 바뀌었다. 기본 `v1` 기능은 예전 API를 유지하지만, 문서는 필요한 저장소 크레이트만 직접 의존하라고 권한다.
- reqwest는 macOS·Windows의 시스템 프록시 설정을 따른다(미확인: 0.13에서의 정확한 동작은 1단계에서 확인).

## 5. 배포

### macOS
- Tauri가 `.app`과 `.dmg`를 만든다.
- 다른 Mac에서 경고 없이 열리려면 **Developer ID Application 서명 + 공증(notarytool)**이 필요하다. 유료 계정이 있으므로 인증서만 발급하면 된다(현재 키체인에 없음).
- 공증은 App Store Connect API 키 또는 Apple ID + 앱 전용 암호로 한다. Tauri 번들러가 환경 변수(`APPLE_API_KEY` 등)를 받아 자동 처리한다.
- 공증 없이 배포하면 최근 macOS에서는 우클릭-열기 우회가 막혀 있어 시스템 설정에서 직접 허용해야 한다.
- 아키텍처: Apple Silicon 전용(`aarch64-apple-darwin`) 또는 유니버설. 유니버설은 빌드 시간과 용량이 두 배다.

### Windows
- NSIS 설치 파일(`-setup.exe`)이 기본이다. WebView2는 Windows 10/11에 대부분 기본 설치되어 있고, 없으면 설치기가 부트스트래퍼로 받는다.
- **macOS에서 Windows 빌드**: Tauri는 `cargo-xwin`으로 NSIS 크로스 빌드를 지원하지만 실험적이다. **GitHub Actions `windows-latest` 러너가 확실하다.**
- 서명하지 않으면 처음 실행할 때 SmartScreen 경고가 뜬다("추가 정보 → 실행"으로 넘어갈 수 있음).
- 서명 선택지: Azure Artifact Signing(구 Trusted Signing, 월 과금, 신원 검증 필요. 개인 개발자의 지역별 가입 가능 여부는 미확인) 또는 OV 인증서 구매.

### 자동 업데이트
- `tauri-plugin-updater`: 업데이트 파일을 자체 키로 서명하고, GitHub Releases에 올린 `latest.json`을 확인한다. 코드 서명과는 별개다.
- GitHub Actions의 `tauri-action`이 두 플랫폼 빌드 → 릴리스 업로드 → `latest.json` 생성을 한 번에 해 준다.

## 6. 앱 내부 전용 VPN (WireGuard 유저스페이스)

요구사항: 시스템 전체가 아니라 **이 앱의 트래픽만** WireGuard 터널로 보낸다. WireGuard 설정 파일 가져오기와 Mullvad 계정 연동을 모두 지원한다.

### 방식
OS에 TUN 인터페이스를 만들면 관리자 권한(macOS 네트워크 확장, Windows wintun 드라이버)이 필요하고 시스템 라우팅에 손을 대야 한다. 대신 **앱 프로세스 안에서 전부 처리**한다.

```
앱 TCP 연결 ─▶ smoltcp(유저스페이스 TCP/IP) ─IP 패킷─▶ WireGuard 암호화(gotatun) ─UDP─▶ VPN 서버
```

- 같은 구조로 동작하는 기존 프로젝트: `onetun`(0.3.10, 2024-12 이후 정체) — boringtun + smoltcp. 개념 검증으로 참고만 한다.
- OS 입장에서는 앱이 VPN 서버로 UDP를 보내는 것뿐이라 **권한 상승이 필요 없고, 다른 앱에는 영향이 없다.**

### 크레이트 (버전 확인)
| 용도 | 크레이트 | 버전 | 비고 |
|---|---|---|---|
| WireGuard | **gotatun** | 0.10.1 (2026-10-16) | Mullvad가 유지하는 boringtun 포크, 보안 감사 보고서 공개, MPL-2.0. `noise::Tunn`(지점 간 연결), `tun`/`udp` 트레이트가 공개되어 있다. 메서드 시그니처는 1단계에서 확인. |
| (대안) | boringtun | 0.7.1 (2026-05) | BSD-3. `noise::Tunn::encapsulate/decapsulate` API가 알려져 있다. gotatun API가 맞지 않으면 대안. |
| TCP/IP | smoltcp | 0.14.0 (2026-08) | `no_std` 지향 유저스페이스 스택. |
| DNS 메시지 | hickory-proto | 0.26.3 | 쓰지 않았다. smoltcp DNS 소켓으로 충분했다(7절). |
| 키 생성 | x25519-dalek | 3.0.0 | 직접 의존하지 않는다. gotatun이 재노출하는 `gotatun::x25519`를 쓴다. |

### Mullvad API (mullvadvpn-app 소스 `mullvad-api`에서 확인)
- 서버 목록: `GET https://api.mullvad.net/app/v1/relays` — 인증 불필요. **이 네트워크에서 접속됨(200, 약 600KB).**
  - `wireguard.relays[]`: `hostname`, `location`, `active`, `ipv4_addr_in`, `ipv6_addr_in`, `public_key`, `daita` 등. 이번 조회 시 553개.
  - `wireguard.ipv4_gateway`: `10.64.0.1` (터널 안 DNS 겸 게이트웨이), `port_ranges`
  - `locations`: 도시/국가 이름
- 토큰: `POST auth/v1/token` 본문 `{"account_number": "…"}` → `access_token`(만료 있음). `Authorization: Bearer <토큰>`으로 붙인다(기기 등록이 성공해 확인).
- 기기(키) 등록: `accounts/v1/devices`
  - 생성 `POST` `{pubkey, hijack_dns: false}` → 201, `{id, name, pubkey, ipv4_address, ipv6_address, created}` — 터널 주소를 여기서 받는다.
  - 목록 `GET`, 조회 `GET /{id}`, 삭제 `DELETE /{id}` → 204, 키 교체 `PUT /{id}/pubkey`
- Mullvad 계정당 기기 수 제한이 있다(일반적으로 5대로 알려짐, 미확인). 앱이 기기 하나를 차지하므로 로그아웃 시 기기를 지운다.
- 계정 정보: `GET accounts/v1/accounts/me`(Bearer) → `expiry`(RFC 3339). 실제 계정으로 확인(2026-10-21).
- 오류 응답은 `{"code": …}`. 직접 확인(2026-10-21): 없는 계정 번호 → 400 `INVALID_ACCOUNT`, 9자리보다 짧은 번호 → 400 `INVALID_INPUT`, 잘못된 토큰 → 401 `INVALID_ACCESS_TOKEN`. 기기 수 초과 `MAX_DEVICES_REACHED`, 없는 기기 삭제 `DEVICE_NOT_FOUND`는 `mullvad-api` 소스의 이름이고 실제로 받아 보지는 않았다.
- 서버 목록은 `ETag`를 주고 `If-None-Match`에 304(본문 0바이트)로 답한다(2026-10-21 확인). 앱은 필요한 필드만 추려 저장하므로 디스크 사본은 약 90KB다.

### 웹뷰 트래픽 (처음 계획 → 폐기)
처음 계획은 웹뷰에 프록시를 지정해(`proxy_url`, macOS 14+와 `mac-proxy` 필요) 웹뷰도 앱의 SOCKS를 타게 하는 것이었다. 실제로 웹뷰 요청이 SOCKS로 들어오는 것까지는 확인했다. 그러나 static1의 핫링크 차단 때문에 웹뷰 요청은 프록시와 상관없이 403이었다(7절). 그래서 미디어도 Rust가 받아 `media://` 커스텀 프로토콜로 넘기는 방식으로 바꾸고, 웹뷰 프록시는 없앴다.
- "커스텀 프로토콜은 응답 본문을 스트리밍하지 못해 동영상에 맞지 않는다"는 우려는 Range 요청을 상위로 그대로 넘기면 피할 수 있다. 다만 아래 7절의 작은 Range 문제가 있다.

## 7. 실측 결과 (2026-10-19)

### 앱 내장 터널 (macOS, Mullvad `jp-tyo-wg-001`)
- gotatun 0.10.1의 `DeviceBuilder::with_ip_pair`에 직접 만든 `IpSend`/`IpRecv`를 꽂으면 OS TUN 없이 동작한다. 핸드셰이크·타이머·대기 패킷 처리는 gotatun이 한다. `device` 기능이 필요하고, 암호 백엔드는 기본값(`aws-lc-rs`)이다. reqwest 0.13의 기본 TLS도 aws-lc-rs라 하나로 통일된다. Windows 빌드에서 aws-lc-rs가 문제없는지는 미확인.
- smoltcp 0.14: `poll()` 한 번에 대기 패킷을 모두 처리한다. `async` 기능의 소켓 waker로 `AsyncRead/AsyncWrite` 스트림을 만들 수 있다. DNS 소켓이 있어 터널 안 DNS에 별도 라이브러리가 필요 없다.
- 시스템 VPN을 끄고 e621 직접 접속이 막힌 상태에서:
  - Mullvad 출구 확인 OK, **e621 API 200 (364ms)**
  - Cloudflare 10MB: 직접 9.4 MB/s, 앱 터널 5.2 MB/s. 직접은 국내 Cloudflare, 터널은 도쿄 경유라 같은 조건의 비교는 아니다.
  - static1 원본 파일은 직접이든 터널이든 0.2~0.4 MB/s로 느렸다. 서버 쪽 사정으로 보인다.
- 터널 시작에서 대부분의 시간(3~4초)은 Mullvad 서버 목록(약 600KB)을 받는 데 쓴다. gotatun 장치를 만드는 데는 수 ms가 걸렸다. **이 수치에는 핸드셰이크가 들어 있지 않다**(8절: `build()`는 핸드셰이크를 기다리지 않는다). 핸드셰이크까지 포함한 시간은 아래 "터널 시작과 static1 속도"에 있다.
- `keyring-core` 1.0 + `apple-native-keyring-store`는 `keychain` 기능을 켜야 컴파일된다(`protected`는 데이터 보호 키체인이라 서명·엔타이틀먼트가 필요). `security add-generic-password`로 만든 항목을 앱이 읽으면 macOS가 접근 허용 창을 띄운다.
- `wireguard-netstack`(GPL-3.0)은 gotatun 0.2·smoltcp 0.12에 묶여 있고 2026-02 이후 릴리스가 없어 쓰지 않았다.

### static1 핫링크 차단 (직접 연결에서도 같음)
- 다음 세 헤더가 **함께** 오고 e621 Referer가 없으면 403이다: `Sec-Fetch-Dest: image`(동영상은 `video`), `Sec-Fetch-Mode: no-cors`, `Sec-Fetch-Site: cross-site`. 하나씩, 또는 `Sec-Fetch-Site: same-site`이면 200이다.
- 이 셋은 웹뷰의 `<img>`/`<video>`가 자동으로 보내는 헤더이고, 페이지 출처(`tauri://localhost`, `http://localhost:1420`)는 늘 cross-site다. 웹 코드로는 바꿀 수 없다.
- Referer를 `https://e621.net/`로 위조하면 통과하지만 쓰지 않는다. API 클라이언트로서 자기 UA로 파일을 받는 것이 정상 경로이므로 Rust가 받는다.

### `media://`에서의 동영상
- WKWebView는 커스텀 프로토콜의 MP4를 8~24바이트짜리 Range로 수백 번 순차적으로 읽는다(MP4 구조 파싱). 요청마다 상위로 왕복하면 각각 약 145ms(도쿄 경유)라 재생이 시작되지 않는다. 이미지(Range 없음)는 문제없다.
- 해결: 1MB 블록 캐시(report.md 7.2절). 빈 캐시·터널에서 재생 시작 2.6초(720p 샘플 1.6MB), 3.05초(3분 MP4 21.9MB). 작은 Range는 블록이 도착하면 수 ms 간격으로 이어진다.
- 창이 다른 창에 완전히 가려지면 WKWebView는 미디어 요청과 페이지 타이머를 멈춘다. 첫 측정에서 `canplay` 3초 뒤 `playing`이 45초에 찍힌 것은 이 때문이었고, 창을 캡처하는 순간 다시 움직였다. 재생 시간은 창을 앞에 두고 잰다.

### 터널 시작과 static1 속도 (2026-10-19 2차, 릴리스 빌드, `jp-tyo-wg-001`)
- 핸드셰이크 포함 터널 시작(`Tunnel::start`, 서버 목록 받기 제외): 37회(릴리스 34, 디버그 3) 중 35회가 31~38ms(핸드셰이크 왕복 약 30ms). 99ms 1회(응답 97ms, 재전송 없음), 5.3초 1회(디버그 빌드, 로그 없음. WireGuard 재전송 간격 5초와 맞아 첫 핸드셰이크 패킷 유실로 보이지만 확인 못 함).
- 터널이 올라온 직후 첫 요청(`am.i.mullvad.net`)이 SOCKS `TtlExpired`로 실패한 것이 37회 중 2회(07:45 무렵 연달아). 원인은 아래 "같은 키 터널 충돌"에서 확인했다(2026-10-21).
- 같은 터널에서 Cloudflare 10MB는 3.2~4.9 MB/s, static1 원본(3~7MB)은 0.33~0.98 MB/s(20회).
- 같은 static1 파일을 직접과 터널로 번갈아 받기(curl, `tunnel_probe --serve`의 SOCKS5):

| 파일 | 직접 | 터널 |
|---|---|---|
| 720p 샘플 7.9MB 전체 (3회) | 0.90~1.25 MB/s | 0.63~1.08 MB/s |
| 720p 샘플 앞 20MB Range (3회) | 1.20~1.75 MB/s | 0.76~1.03 MB/s |

  첫 바이트까지는 양쪽 모두 약 0.47초. 느린 주원인은 static1 쪽이고, 터널이 0~50% 정도 더 느리다.
- 720p 샘플 비트레이트: 0.11~0.14 MB/s(4개, 14초~11분짜리). 터널 속도는 그 5~8배다.
- 터널에서 Range 크기별 응답 시간(각 3회): 256KB 1.1~2.1초, 512KB 1.4~1.5초, 1MB 1.8~2.1초, 2MB 2.6~3.6초. 첫 바이트 대기(약 0.5초)와 연결이 크기보다 크게 작용한다.
- 720p 샘플 3개 모두 `ftyp` 다음에 바로 `moov`가 있다(faststart). `moov`는 8KB~480KB(11분짜리가 480KB)라 첫 1MB 안에 들어간다.

### 같은 키 터널 충돌과 응답 없는 서버 (2026-10-21, 디버그 빌드)

임시 반복 프로그램으로 터널을 띄우고 곧바로 터널 안 DNS → TCP 연결(443)을 했다. 매번 새 gotatun 장치·smoltcp 스택이고, smoltcp `log` 기능과 netstack 패킷 로그(`phy::Tracer`)를 켰다.

| 실험 | 결과 |
|---|---|
| 터널 하나, 도쿄 서버 무작위, 회차 사이 2초, 300회 | 터널이 올라온 229회 모두 DNS·연결 성공(DNS 최대 0.3초) |
| 같은 실험, 멈추자마자 다시 연결, 40회 | 터널이 올라온 31회 모두 성공 |
| 같은 키로 터널 A → B를 같은 서버(wg-001)에, 5라운드 | B의 DNS는 매번 약 30ms에 성공, **A의 DNS는 5/5회 10.00초 시간 초과** |
| B가 떠 있는 채 C를 띄움, 5라운드 | **B의 DNS 5/5회 10.00초 시간 초과** |

- 패킷 로그: A는 DNS 질의를 0·1·3·7초(smoltcp 재전송 간격)에 보냈고 netstack에 들어온 패킷은 하나도 없다. 같은 시각 B의 질의는 33ms 만에 응답을 받았다.
- 해석: WireGuard 서버는 피어(공개 키) 하나에 대해 마지막 핸드셰이크로 생긴 세션의 키로만 응답을 암호화한다. 먼저 연결한 터널은 보낼 수는 있어도 응답을 풀지 못하고, gotatun은 모르는 수신 인덱스의 패킷을 로그 없이 버린다(8절). 터널 안 DNS 제한(10초)이 smoltcp 자체 제한(10초)보다 먼저 끝나므로 SOCKS 응답은 `TtlExpired`다. smoltcp·netstack 결함이 아니다.
- 10-19의 2회: 당시 `tunnel_probe`는 늘 목록 첫 서버(wg-001)를 썼고, 같은 날 개발 앱도 같은 기기 키로 wg-001에 연결해 시험했다. 그 시각에 앱이 떠 있었는지는 기록이 없어 단정할 수 없지만, 터널 하나로는 260회 0건, 충돌로는 10/10건이다.
- 앱에서 생길 수 있는 경우: 앱 두 개(Windows 두 번 실행, 개발 앱과 설치본), 앱이 연결된 동안의 probe, 같은 `.conf`를 다른 WireGuard 앱에서도 켜 둔 경우. 두 쪽이 번갈아 핸드셰이크하며(gotatun은 응답 없는 데이터 15초 뒤, 건강 검사는 120초마다) 서로 끊는다. 대응은 report.md 6.6절.
- **`jp-tyo-wg-002`는 핸드셰이크에 한 번도 답하지 않았다**(반복 실행에서 80/80회, 포트를 53·4500·51820으로 바꿔 각 2회도 모두). 같은 조건에서 wg-001은 포트 53에서도 바로 붙었다. 서버는 ping에 답하고, `app/v1/relays`와 `www/relays/wireguard` 모두 `active`, 공개 키도 같다. 다른 도쿄 서버와 달리 `app/v1`에서 `daita`·`features` 항목이 없다. 원인(이 서버에 기기 키가 배포되지 않았는지, WireGuard가 멈췄는지)은 앱 쪽에서는 가릴 수 없다. wg-203도 1/65회 답하지 않았다. 대응은 report.md 6.2절(답하지 않은 서버 빼기).

### `media://` 다시 요청과 디스크 캐시 (2026-10-21, 디버그 빌드, 터널)

- 실행 중 다시 요청: 최신 게시물 200개를 받고 150번째까지 내려갔다가 맨 위로 돌아왔다(임시 코드, `media <경로>` 디버그 로그). 요청 108건이 모두 서로 다른 경로였고, 돌아온 뒤에는 0건이었다(첫 화면 42건, 내려갈 때 66건). 맨 위의 썸네일은 그대로 보였다. **WKWebView는 한 실행 안에서 받은 `media://` 이미지를 다시 요청하지 않는다.** 그래서 디스크 캐시의 효과는 다시 실행한 뒤에만 있다. 오래 쓰다 메모리 압박으로 WebKit이 버리는 경우는 재지 않았다.
- 디스크 캐시를 넣은 뒤: 첫 실행에서 첫 화면 썸네일 42개를 받아 42개 파일(1.2MB)이 생겼다. 앱을 다시 켜자 첫 화면 42건 중 41건을 디스크에서 응답했다(나머지 1건은 그사이 올라온 새 게시물로 보임). [비우기]로 `media/` 폴더가 지워지고 같은 캐시 폴더의 `relays.json`은 남았다.

## 8. 의존성 동작 확인 (2026-10-19 재검토)

코드와 문서의 전제를 의존성 소스(`~/.cargo/registry`의 해당 버전)와 실측으로 대조했다. 경로는 각 크레이트의 `src/` 기준이다.

### gotatun 0.10.1
- `DeviceBuilder::build()`는 UDP 소켓을 열고 작업을 띄운 뒤 바로 돌아온다. **핸드셰이크를 기다리지 않는다**(`device/builder.rs` `build`, `device/mod.rs` `Connection::set_up`). 서버에 닿지 않아도 성공한다.
- `Device::force_handshake(&public_key)`: 유효한 세션이 있으면 바로 돌아오고, 없으면 핸드셰이크를 시작해 끝날 때까지 기다린다. 시간 제한이 없으므로 호출하는 쪽이 건다(`device/mod.rs`).
- `Device::wait()`: 복구할 수 없는 오류가 날 때까지 기다린다. 피어 통계 `Stats.last_handshake`는 마지막 핸드셰이크 뒤 지난 시간이고, 세션이 없으면 `None`이다(`device/configure.rs`, `noise/timers.rs` `time_since_last_handshake`, `Device::read()`로 읽음).
- **재키잉은 데이터 패킷을 보낼 때만 일어난다**: 개시자는 세션이 `REKEY_AFTER_TIME`(120초)을 넘고 그 뒤 데이터 패킷(빈 keepalive 제외)을 보냈을 때 새 핸드셰이크를 건다(`noise/timers.rs` `update_timers`). 그래서 keepalive만 오가는 쉬는 터널은 `last_handshake`가 180초를 정상적으로 넘긴다. 이 값만으로는 끊김을 판단할 수 없어, 120초가 넘으면 `force_handshake`로 직접 확인한다(report.md 6.3절 5).
- 데이터를 보냈는데 `KEEPALIVE + REKEY_TIMEOUT`(15초) 동안 응답이 없으면 gotatun이 스스로 핸드셰이크를 건다. 세션 키는 `REJECT_AFTER_TIME * 3`(540초) 뒤 지워진다.
- 핸드셰이크 재시도 간격 `REKEY_TIMEOUT`은 5초(+지터), 시도 한도 `REKEY_ATTEMPT_TIME`은 90초(`noise/timers.rs`).
- `IpRecv::recv`가 오류를 돌려주면 송신 루프가 "Unexpected error on tun interface"를 남기고 끝난다(`device/mod.rs` `handle_outgoing`). gotatun 자체 TUN 리더는 파싱할 수 없는 패킷을 오류로 돌려준다(`tun/tun_async_device.rs`). 그래서 우리 어댑터(`wg.rs`)는 이상한 패킷을 오류로 올리지 않고 버린다.
- `Device`를 `stop()` 없이 버리면 Drop이 정지 작업을 띄운다.
- 받은 패킷의 수신 인덱스가 이 장치의 세션에 없으면 로그 없이 버린다(`device/mod.rs` `handle_incoming`의 `peers_by_idx` 조회). 복호화 오류(`TunnResult::Err`)도 그냥 넘어간다. 같은 키를 다른 곳에서 쓸 때 응답이 사라지는 지점이다(7절).
- `force_handshake`의 완료 신호는 핸드셰이크 응답을 받아들였을 때만 온다(`device/peer_state.rs` `handle_incoming_packet`). 세션이 생기기 전에 첫 요청이 나가는 일은 없다.

### smoltcp 0.14.0
- `iface::Config::new`의 `random_seed` 기본값은 **0**이다(`iface/interface/mod.rs`). TCP 초기 순서 번호(`socket/tcp.rs` `connect`)와 DNS 질의 ID·출발 포트(`socket/dns.rs`)가 이 시드에서 나온다. 문서도 부팅마다 다른 값을 쓰라고 권한다.
- TCP `timeout`의 기본값은 없음이다. keep-alive는 패킷만 보낼 뿐이고, 응답이 없을 때 연결을 끊는 것은 `set_timeout`이다(`socket/tcp.rs` `set_timeout`, `set_keep_alive` 문서). **keep-alive만 켜면 상대가 사라져도 연결이 끝나지 않는다.** timeout이 있으면 keep-alive를 켠 연결은 상대 패킷 사이 간격이 timeout을 넘을 때 끊긴다.
- 실측: 우리 `NetStack`에 smoltcp 상대 스택을 붙여 연결한 뒤 상대를 끊었다. keep-alive만 있을 때는 읽기가 75초 넘게 끝나지 않았고, keep-alive 15초 + timeout 45초로 바꾸자 45.0초에 끝났다. 끝날 때 오류가 아니라 EOF(`Ok(0)`)로 보인다.
- 실측: 시드 0이면 새로 만든 두 스택의 첫 SYN 순서 번호가 같았다(`netstack.rs` 테스트).
- `Interface::poll()`은 들어온 패킷이 없을 때까지 처리하고, 보낼 것이 없을 때까지 내보낸다(`iface/interface/mod.rs` `poll`). 7절의 "한 번에 모두 처리"가 맞다.
- `socket-tcp-cubic` 기능이 켜져 있으면 새 소켓의 혼잡 제어 기본값이 Cubic이다(`socket/tcp/congestion.rs` `AnyController::new`).
- 상태가 바뀌면 읽기·쓰기 waker를 모두 깨운다(`socket/tcp.rs` `set_state`). 연결 수립을 쓰기 waker로 기다려도 된다.

### reqwest 0.13.5
- `ClientBuilder::proxy`를 부르면 시스템 프록시 자동 사용이 꺼진다(`async_impl/client.rs` `proxy`). e621 요청이 시스템 프록시로 새지 않는다. Mullvad용 클라이언트는 프록시를 지정하지 않으므로 시스템 프록시를 따른다.
- `connect_timeout`, `read_timeout`(읽기 사이 간격), `timeout`(요청 전체)이 있다.
- 의존성 트리에 `ring`은 없고 암호 백엔드는 aws-lc-rs 하나다(gotatun, rustls). 7절의 "하나로 통일"이 맞다.

### tauri-plugin-single-instance 2.5.4
- macOS는 `/tmp/<번들 ID의 .과 -를 _로>_si.sock` Unix 소켓, Windows는 번들 ID로 이름 붙인 뮤텍스로 이미 실행 중인지 안다. 플러그인 `setup`에서 판단해 두 번째 프로세스는 `exit(0)`하므로 앱의 `setup`(터널 시작) 전에 끝난다(`platform_impl/macos.rs`, `windows.rs`).
- 강제 종료로 소켓 파일이 남아도 다음 실행이 연결을 시도해 `ConnectionRefused`를 받으면 지우고 새로 듣는다. 실측: 앱을 SIGTERM으로 끝낸 뒤 다시 실행하면 정상 시작.
- 실측(macOS): 앱이 떠 있을 때 실행 파일을 다시 실행하면 0초 만에 종료 코드 0으로 끝나고 터널은 하나뿐이다. 다른 앱이 앞에 있을 때 터미널에서 다시 실행하면 창이 앞으로 오지 않았다. tao의 `set_focus`는 `activateIgnoringOtherApps`를 부르는데(`platform_impl/macos/util/async.rs`), macOS 14부터 이 요청은 시스템이 거절할 수 있다(협조적 활성화). 원인 확인은 하지 않았다. Finder·Dock에서 이미 실행 중인 `.app`을 열면 macOS가 새 프로세스 없이 기존 앱을 앞으로 가져온다.

### wry 0.57.0
- macOS 커스텀 프로토콜 응답에 본문 길이로 `Content-Length`를 직접 넣는다(`wkwebview/class/url_scheme_handler.rs`).

### keyring-core 1.0.1
- 오류의 `Display`는 비밀 값 바이트를 넣지 않는다(`error.rs`). 로그에 오류를 그대로 남겨도 된다.
- `mock` 저장소는 같은 서비스·이름의 항목을 공유하고, `Cred::set_error`로 다음 호출 하나를 실패시킬 수 있다(`mock.rs`). 키체인 오류 경로를 테스트하는 데 쓴다.

### Tailwind CSS 4.3.3
- 테마 반경을 `--radius-s`, `--radius-l`로 지으면 `rounded-s`, `rounded-l`이 테마 값 규칙과 내장 규칙(논리적 시작 쪽, 왼쪽 두 모서리)으로 **두 번** 생성되고, 뒤에 오는 내장 규칙이 일부 모서리를 0.25rem으로 덮는다. WKWebView에서 계산값을 재 보니 `rounded-l` 패널은 왼쪽 위·오른쪽 위·왼쪽 아래·오른쪽 아래가 4/14/4/14px, `rounded-s`는 4/6/4/6px이었다. `--radius-m`은 겹치는 내장 이름이 없어 9px로 맞았다.

### WKWebView (macOS 27.0.1)
- `document.startViewTransition`이 있다(2026-10-21 실제 앱에서 확인). 썸네일 → 뷰어 전환에 쓴다. 기능 검사는 그대로 둔다(report.md 8절).
- `navigator.language`는 앱 번들의 지역화 선언과 관계없이 시스템 언어를 돌려준다. 시스템 언어가 `ko-KR`일 때 번들 없는 실행 파일, Tauri식 번들(`CFBundleDevelopmentRegion`만 있음), `CFBundleLocalizations`에 ko를 넣은 번들 셋 다 `ko-KR`이었다. `Bundle.main.preferredLocalizations`는 앞의 둘에서 `en`이었으므로 웹뷰 언어와는 별개다.
