# Tailgate 구현 계획 보고서

Windows·macOS용 e621 뷰어. 다른 사람에게도 배포한다. 근거 자료는 [research.md](research.md)와 Pawru 조사 자료다.

정해진 사항:
- Tauri 2 + Svelte 5 + Tailwind CSS v4
- 기본 언어 영어, 영어·한국어 지원
- 다른 사람에게도 배포 (공개 릴리스, 자동 업데이트)
- Windows 미서명, macOS는 Apple Silicon 전용 + Developer ID 서명·공증
- **앱 내부 전용 VPN**: WireGuard 설정 파일 또는 Mullvad 계정으로, 이 앱의 트래픽만 터널로 보낸다. 개발 중 API 검증은 시스템 VPN으로 한다.
- 미디어는 Rust가 받아 `media://`로 웹뷰에 넘긴다 (static1 핫링크 차단, research.md 7절)

## 1. 한 줄 요약

**Tauri 2(Rust) + Svelte 5(TypeScript) + Tailwind CSS v4 + 앱 내장 WireGuard 터널**. API 호출·스로틀·인증·블랙리스트·다운로드·터널, 그리고 **미디어 파일 받기**까지 Rust가 맡는다. 웹뷰는 화면을 그리고, Rust가 넘겨준 미디어를 재생만 한다. 앱의 외부 요청은 모두 Rust에서 나가 앱 안의 SOCKS5 프록시와 (설정했다면) 터널을 거친다.

이렇게 나누는 이유:
- **API는 Rust에서만 부른다.** 웹뷰가 부르면 브라우저 UA가 나가서 e621 규칙(전용 UA)을 어기고, 스로틀을 우회하는 경로가 생긴다. API 키도 웹뷰 메모리에 올라가지 않는다.
- **미디어도 Rust가 받아 `media://` 커스텀 프로토콜로 웹뷰에 넘긴다.** 처음에는 웹뷰가 static1을 직접 받게 했지만, static1이 웹뷰의 cross-site 요청을 핫링크로 보고 403을 준다(research.md 7절). 웹 페이지는 그 헤더를 바꿀 수 없다. GIF 애니메이션과 동영상 디코딩은 여전히 웹뷰가 한다.

## 2. 기능 범위

### 1단계 (MVP)
- **계정**: 사용자명 + API 키 로그인, `/users/me.json`으로 확인, 로그아웃. 로그인 없이도 둘러보기 가능.
- **검색**: 태그 입력(자동완성, 입력한 태그를 카테고리 색으로 표시), 정렬 메뉴, 최근 검색어.
- **그리드**: 원본 비율을 유지하는 행 정렬(justified) 레이아웃, 가상 스크롤, 무한 스크롤, 썸네일 크기 조절(Ctrl/⌘ + 휠 또는 슬라이더).
- **뷰어**: 창 전체를 덮는 뷰어. 이미지는 샘플을 먼저 띄우고 원본은 확대하거나 요청할 때 받음. 줌·패닝, GIF, 동영상(MP4 우선) 반복 재생. 오른쪽 패널에 메타 정보와 카테고리별 태그.
- **키보드**: ←/→ 이전·다음, Esc 닫기, F 즐겨찾기, ↑/↓ 투표, / 검색창, D 다운로드.
- **즐겨찾기·투표**, 내 즐겨찾기 목록.
- **블랙리스트**: 계정의 블랙리스트를 Rust에서 평가해 걸러 냄. "N개 숨김"을 누르면 잠시 보이게 함.
- **다운로드**: 원본을 지정 폴더에 저장(파일명 규칙 설정 가능, 예: `{id}_{artist}.{ext}`). 저장 대화상자 대신 기본 폴더로 바로 저장.
- **연결**: 직접 연결 / WireGuard 설정 파일(.conf 가져오기) / Mullvad 계정(서버 위치 선택). 터널 상태를 사이드바 아래에 표시.
- **설정**: 계정, 다운로드 폴더와 파일명 규칙, 썸네일 크기, 캐시 비우기.
- **자동 업데이트**: 다른 사람에게 배포하므로 1단계에 넣는다.

### 2단계
- 태그 클릭으로 재검색, 우클릭 메뉴(검색에 추가/제외/새 탭에서 검색)
- 검색 탭 여러 개(창 상단 탭)
- Pool 목록과 순서대로 읽는 리더
- Set 목록·열람·추가/빼기
- 댓글과 DText 렌더링
- 인기 게시물

### 3단계 (필요해지면)
- 저장한 검색(사이드바 고정), 댓글 작성, 일괄 다운로드(풀 전체 등, API 스로틀 준수)

## 3. 구조

Tauri 기본 구조에 폴더만 나눈다. Cargo 워크스페이스나 크레이트 분리는 하지 않는다. 빌드가 느려지면 그때 `api`를 크레이트로 뗀다. ✅는 2026-10-19 현재 구현된 것이다.

```
tailgate/
  src-tauri/
    src/
      main.rs            진입점
      lib.rs             Tauri 빌더, 창 생성(플랫폼별 옵션), 상태 조립, 이벤트 연결     ✅
      commands.rs        프런트엔드에 노출하는 명령 (얇게: 검증 → 호출 → 반환)          ✅ connection_status
      network.rs         SOCKS 프록시 + 터널 소유, 연결 상태(watch), e621용 HTTP 클라이언트 ✅
      media.rs           media:// 프로토콜: static1 data/ 경로만, 터널 경유로 받아 응답   ✅ (동영상 블록 캐시 남음)
      secrets.rs         OS 키체인 (keyring-core + 플랫폼 저장소)                      ✅
      tunnel/
        config.rs        WireGuard .conf 파서 → TunnelConfig (+ 테스트)               ✅
        mullvad.rs       토큰, 기기 등록/삭제, 서버 목록 → TunnelConfig               ✅
        wg.rs            gotatun Device에 smoltcp 패킷 큐를 꽂는 IpSend/IpRecv 어댑터  ✅
        netstack.rs      smoltcp 인터페이스, async TcpStream, 터널 안 DNS              ✅
        socks.rs         127.0.0.1 SOCKS5 서버, Route(Direct/Tunnel/Blocked)           ✅
        mod.rs           Tunnel::start/stop (밖에서는 이것만 쓴다)                     ✅
      error.rs           앱 오류 타입 (프런트엔드에는 오류 코드와 값만 보낸다. 문구는 프런트엔드 몫)
      api/
        client.rs        e621 요청, UA, Basic 인증, 스로틀, 재시도
        models.rs        응답 모델 (serde)
        endpoints.rs     posts, favorites, votes, users/me, autocomplete …
      blacklist.rs       파서 + 평가기 (+ 단위 테스트)
      account.rs         API 키 저장/삭제, 현재 사용자 상태
      settings.rs        설정 JSON (앱 설정 디렉터리)
      download.rs        원본 다운로드, 파일명 규칙
    examples/
      tunnel_probe.rs    실제 Mullvad 서버로 터널을 검증하는 수동 점검 도구            ✅
    tests/fixtures/      Pawru 픽스처 재사용 (실제 응답 JSON)
  src/                   Svelte 5 + TypeScript
    lib/
      connection.svelte.ts  연결 상태 (명령 + 이벤트 구독)                           ✅
      media.ts           static1 URL → media:// 주소 (+ 썸네일/샘플/원본/동영상 선택)    ✅ 주소 변환만
      platform.ts        OS 판별, 단축키 표기                                         ✅
      i18n/              en.ts(기준), ko.ts, 현재 언어 상태                           ✅
      views.ts           사이드바 화면 목록                                           ✅
      ipc.ts             invoke 래퍼 (명령 이름·타입을 한곳에)
      layout.ts          justified 행 계산 (순수 함수)
      keys.ts            키보드 단축키
    state/               검색, 계정, 설정 상태 (Svelte runes 모듈)
    views/               Search, Viewer, Favorites, Settings, Login
    components/          Sidebar, SearchField, EmptySearch ✅ / Grid, Thumb, TagInput, TagList …
    styles/app.css       Tailwind 진입점, 디자인 토큰(CSS 변수 + @theme), 기본 스타일   ✅
```

원칙:
- **로직은 Rust, 화면은 Svelte.** 블랙리스트, 페이지네이션 커서, 응답 정규화, DText 파싱(2단계)은 Rust에서 하고 `cargo test`로 고정한다. 프런트엔드 테스트는 `layout.ts`처럼 순수 함수만 둔다.
- Repository 계층, DI 컨테이너, 상태 관리 라이브러리는 쓰지 않는다. 상태는 Svelte runes 모듈 몇 개면 충분하다.
- Rust↔TS 타입은 처음에는 손으로 맞춘다(모델이 10개 안팎). 어긋나는 일이 생기면 `ts-rs`로 생성하는 방식으로 바꾼다.

## 4. 데이터 흐름

```
Svelte 화면 → ipc.ts → invoke("search_posts", {tags, cursor})
  → commands.rs → api::client
      스로틀(tokio Mutex로 직렬화, 최소 1초 간격)
      UA "Tailgate/0.1 (by <user> on e621)", Authorization: Basic …
      401 → 로그아웃 상태 전환 / 429·503 → Retry-After 또는 지수 백오프(최대 3회) / 410 → 끝
  → serde 디코딩 → 블랙리스트 평가 → { posts, hidden_count, next_cursor }
  → 화면이 <img src="media://localhost/data/…">로 표시 (Windows는 http://media.localhost/data/…)
  → media.rs가 터널 경유로 static1에서 받아 응답
```

- **커서**: 태그에 `order:`가 없으면 `b<마지막 id>`, 있으면 `page=N`. 계산은 Rust가 하고 프런트엔드는 불투명한 커서 문자열만 돌려준다.
- **한 페이지가 전부 블랙리스트로 숨겨지면** Rust가 다음 페이지를 이어 받는다(최대 횟수 제한).
- **중복 제거**: 받은 id 집합으로 거른다.
- **미디어 선택**(`media.ts` 한곳): 썸네일 `preview` → 그리드 행 높이가 크면 `sample`, 뷰어는 `sample` → 원본은 요청/확대 시 `file`. 동영상은 `720p` → `480p` → `variants.mp4` → 원본 WebM 순. URL이 null이면 플레이스홀더.
- **원격 미디어는 연결이 준비된 뒤에만 요청한다.** 터널 모드에서 준비 전 요청은 킬 스위치에 막혀 실패하고, 웹뷰가 실패를 기억해 다시 요청하지 않는다. 화면은 `connection.svelte.ts`의 `canLoadRemote()`를 보고 렌더링한다.
- `media://`는 static1의 `data/` 경로만 받는다. 다른 경로는 404로 거절해 임의 URL 요청 통로가 되지 않게 한다.

## 5. 앱 내부 전용 VPN

```
            ┌──────────────────── Tailgate 프로세스 ─────────────────────────────────┐
 웹뷰 ──media://──▶ media.rs ─┐                                                     │
            │                 ├─▶ reqwest ─socks5h─▶ socks.rs ─▶ netstack.rs(smoltcp) ─IP─▶ wg.rs ⇄ gotatun Device ─┼─UDP─▶ WireGuard 서버 ─▶ e621
 화면 ─invoke─▶ api(예정) ─────┘                                                     │
            └──────────────────────────────────────────────────────────────────────┘
```

- **권한 상승이 필요 없다.** OS에 VPN 인터페이스를 만들지 않으므로 다른 앱과 시스템 라우팅에 영향이 없다.
- **WireGuard 프로토콜 처리(핸드셰이크·재키잉·keepalive)는 gotatun의 `Device`가 한다.** OS TUN 대신 `with_ip_pair`로 smoltcp와 패킷을 주고받는 어댑터(`wg.rs`)만 우리가 쓴다.
- **웹뷰는 외부로 직접 요청하지 않는다.** 그래서 웹뷰 프록시 설정(`proxy_url`)이 필요 없고, 루프백 SOCKS는 Rust 내부용이다.
- **연결 방식 세 가지**(설정 > 연결, 1단계 9번에서 화면 구현)
  - 직접 연결: SOCKS가 그냥 직접 연결한다.
  - WireGuard 설정 파일: 표준 `.conf`를 가져온다(`PrivateKey`, `Address`, `DNS`, 피어의 `PublicKey`, `PresharedKey`, `Endpoint`, `PersistentKeepalive`). `AllowedIPs`는 앱 트래픽 전체를 보내므로 무시한다. 파서는 구현·테스트 완료.
  - Mullvad: 계정 번호 → 토큰 → 앱이 키 쌍을 만들어 기기로 등록 → 받은 터널 주소와 서버 목록으로 설정을 만든다. 서버는 국가/도시로 고른다(지금은 `jp-tyo` 고정). 로그아웃하면 기기 등록을 지운다.
- **지금의 임시 규칙**: 키체인에 Mullvad 기기(`mullvad-device`)가 있으면 터널, 없으면 직접 연결. 설정 화면이 생기면 사용자가 고른 방식으로 바꾼다.
- **킬 스위치가 기본이다.** 터널 모드에서는 시작부터 `Blocked`이고, 터널이 올라온 뒤에만 `Tunnel`로 바뀐다. 실패하면 `Blocked`로 남고 연결 상태가 `failed`가 된다. 직접 연결로 넘어가는 경로는 없다.
- **연결 상태**: `direct` / `connecting` / `tunnel {relay}` / `failed {reason}`. Rust가 `watch` 채널로 갖고, 바뀌면 `connection-changed` 이벤트를 보낸다. 화면은 `connection_status` 명령으로 첫 값을 읽는다.
- **DNS도 터널 안에서 푼다.** SOCKS5에 도메인 이름째로 넘기고(`socks5h`), 설정의 `DNS`(Mullvad는 `10.64.0.1`)로 smoltcp DNS 소켓이 조회한다.
- **Mullvad API 호출 자체는 직접 연결로 한다.** 터널을 세우기 위한 호출이기 때문이다.
- **비밀 값**: WireGuard 개인 키(기기 정보 JSON 안)와 Mullvad 계정 번호는 OS 키체인에 저장한다. 서비스 이름은 `pro.tb.nyabi.Tailgate`, 항목은 `mullvad-account`, `mullvad-device`.
- 터널은 IPv4만 다룬다(smoltcp 인터페이스, DNS A 레코드).

## 6. 캐시와 저장

| 대상 | 방식 |
|---|---|
| 이미지·동영상 | `media.rs`가 관리한다. 커스텀 프로토콜에는 웹뷰 HTTP 캐시가 적용되지 않으므로 직접 캐시해야 한다. 다음 할 일: 1MB 정렬 블록 메모리 캐시(13절). 이후 디스크 캐시로 확장하고 설정에 용량 한도·비우기를 둔다. |
| API 응답 | 캐시하지 않는다. 즐겨찾기·점수는 항상 최신이어야 한다. |
| 블랙리스트 | 로그인 시와 창이 다시 활성화될 때 갱신하고 설정 파일에 마지막 값을 저장(오프라인으로 시작해도 필터 적용). |
| 자격 증명 | OS 키체인(macOS Keychain, Windows 자격 증명 관리자). 설정 파일·로그·URL에 키를 남기지 않는다. |
| 설정·최근 검색어 | 앱 설정 디렉터리의 JSON 파일 하나. |

처음 계획은 웹뷰 HTTP 캐시에 맡기는 것이었지만, 핫링크 차단 때문에 미디어가 `media://`로 옮겨 오면서 캐시도 앱이 갖게 됐다. 용량 한도를 앱이 정할 수 있게 된 것은 덤이다.

## 7. 디자인

### 방향
e621은 색이 강한 그림이 수천 장 쏟아지는 곳이다. **색은 그림이 갖고, 앱은 물러선다.** 앱이 쓰는 강한 색은 하나의 의미에만 쓴다: **태그 카테고리**(아티스트, 캐릭터, 종 …). e621 사용자가 이미 아는 색-카테고리 대응을 따르되 색값은 새로 조율한다. 검색창에 친 태그도, 뷰어의 태그 목록도 같은 색 체계를 쓰므로 색이 장식이 아니라 정보가 된다.

### 토큰 (시스템 설정을 따라 라이트/다크)
스타일은 **Tailwind CSS v4**로 쓴다. 토큰은 `src/styles/app.css`의 CSS 변수에 두고 `@theme inline`으로 유틸리티(`bg-panel`, `text-dim`, `text-tag-species` …)에 연결한다. Tailwind 기본 색 팔레트는 지워서 토큰 밖의 색을 쓰지 못하게 한다. 유틸리티로 표현하기 어색한 곳(그리드 배치, 전환 애니메이션)만 컴포넌트 `<style>`을 쓴다.

| 이름 | 다크 | 라이트 | 쓰임 |
|---|---|---|---|
| window | `#17191C` | `#E9EBEE` | 창 바탕 (macOS에서는 반투명 재질이 대신함) |
| panel | `#1E2024` | `#FFFFFF` | 본문 패널 |
| raised | `#272A2F` | `#F0F1F3` | 입력창 바탕 |
| hover / selected | 흰색 6% / 10% | 검정 4.5% / 7.5% | 목록 항목 상태 |
| seam | 흰색 7% | 검정 8% | 구분선 |
| ink / dim / faint | `#ECEEF1` / `#8D929B` / `#5D626B` | `#1A1C20` / `#696E77` / `#A0A4AB` | 본문 / 보조 / 힌트 |
| focus | `#7AA2E8` | `#2F5FB8` | 포커스, 선택된 항목의 아이콘 |

### 글꼴
- **Wanted Sans Variable** 하나만 쓴다(OFL, 한글·라틴 모두 포함, 앱에 번들). 숫자는 tabular-nums로 점수·ID 열을 맞춘다.
- 크기: 12 / 13 / 15 / 19 / 26px. 데스크톱 앱이라 본문 13px. 굵기는 400·520·650 세 단계.
- 대문자 라벨, 고정폭 글꼴 라벨은 쓰지 않는다.

### 레이아웃
```
┌─────────┬──────────────────────────────────────────────┐
│ ● ● ●   │ [artist:foo] [wolf] [-comic]          최신순 ▾ │  ← 창 제목줄과 합친 상단 바
│         ├──────────────────────────────────────────────┤
│ 검색    │ ▇▇▇▇▇ ▇▇▇▇▇▇▇▇ ▇▇▇▇ ▇▇▇▇▇▇                   │
│ 즐겨찾기│ ▇▇▇▇▇▇▇▇ ▇▇▇▇ ▇▇▇▇▇▇▇ ▇▇▇▇                   │  ← 원본 비율 유지, 틈 3px
│ 인기    │ ▇▇▇▇ ▇▇▇▇▇▇▇ ▇▇▇▇▇ ▇▇▇▇▇▇▇                   │
│ 풀      │                                              │
│ 세트    │                                12개 숨김 보기 │
│         │                                              │
│ 최근    │                                              │
│  wolf   │                                              │
│  …      │                                              │
│ ─────── │                                              │
│ ● 도쿄  │                                              │  ← 연결 상태 (직접 / 터널 서버 위치)
│ 설정    │                                              │
└─────────┴──────────────────────────────────────────────┘

뷰어 (그리드 위를 덮음)
┌──────────────────────────────────────┬─────────────────┐
│                                      │ #4123456        │
│                                      │ 점수 312  ♥ 1,204│
│            [ 이미지 / 동영상 ]         │ [♥] [▲] [▼] [↓] │
│                                      │                 │
│  ‹                                ›  │ 아티스트         │
│                                      │  foo            │
│                                      │ 캐릭터 …         │
│                                      │ 종 …            │
└──────────────────────────────────────┴─────────────────┘
```
- 모든 텍스트는 왼쪽 정렬. 사이드바 폭 220px, 뷰어 정보 패널 300px(접을 수 있음).
- **본문은 창 안에 떠 있는 패널**(바깥 여백 8px, 반경 14px, 얇은 테두리 그림자)이고, 사이드바는 그 뒤의 창 바탕에 놓인다.
- **macOS**: 창을 투명하게 하고 사이드바 자리에 시스템 `sidebar` 재질(vibrancy)을 깐다(`lib.rs`의 `build_main_window`). 신호등 버튼은 검색창과 같은 높이에 맞춘다(`x 20, y 33`).
- **Windows**: 지금은 불투명한 `window` 색. Windows 11 Mica는 실기기에서 확인한 뒤 넣는다.
- 아이콘은 Lucide(16px, 선 굵기 1.75). 선택된 항목은 바탕을 채우고 아이콘만 focus 색으로 바꾼다.
- 검색창은 아이콘과 단축키 표시(⌘K / Ctrl K)를 갖는다. ⌘K, Ctrl K, `/`로 어디서든 검색창으로 간다.
- 빈 검색 화면은 제목, 사용법 한 줄, 누르면 검색어에 들어가는 예시 태그로 이뤄진다. 예시 태그는 카테고리 색을 써서 첫 화면부터 색 체계를 보여 준다.
- 썸네일은 카드가 아니다. 테두리·그림자·둥근 모서리 없이 틈만 둔다. GIF/동영상/즐겨찾기 여부는 모서리에 작은 기호 하나로만 표시한다.
- macOS는 투명 제목줄(신호등 버튼이 사이드바 위에 얹힘), Windows는 기본 제목줄을 쓴다. Windows 커스텀 제목줄은 스냅 레이아웃 등 OS 동작을 다시 만들어야 해서 쓰지 않는다.

### 움직임
- 사용자가 일으키지 않는 애니메이션은 없다.
- 하나만 공들인다: **썸네일에서 뷰어로 열리고 닫힐 때** 썸네일 자리에서 커지는 전환(View Transitions API, 두 웹뷰 모두 지원). `prefers-reduced-motion`이면 즉시 전환.

### 자기 검토
첫 구현(사이드바와 본문을 탁한 회색 판 두 장으로 나누고 아이콘 없이 글자만 둔 화면)은 와이어프레임처럼 투박하다는 피드백을 받았다. 위계가 없고 재질감이 없어서였다. 그래서 반투명 사이드바, 떠 있는 본문 패널, 아이콘, 검색창 상태 표현을 더했다. 그 전에 처음 떠올린 안은 "거의 검은 배경 + 형광 강조색 하나 + 둥근 카드 그리드"였다. 이미지 뷰어라면 누구나 내놓을 기본값이라 버렸다. 대신 (1) 배경을 순흑이 아닌 차가운 회청색으로 해서 그림의 색이 틀어져 보이지 않게 하고, (2) 강조색을 하나 두는 대신 태그 카테고리 색을 정보로 쓰고, (3) 카드를 없애고 그림 자체가 격자를 이루게 했다.

## 8. 배포

| | macOS | Windows |
|---|---|---|
| 산출물 | `.dmg` (Apple Silicon, macOS 14 이상) | NSIS `-setup.exe` (x64, Windows 10/11) |
| 빌드 위치 | GitHub Actions `macos-latest` (로컬도 가능) | GitHub Actions `windows-latest` |
| 서명 | Developer ID Application + 공증 | 미서명 (첫 실행 시 SmartScreen 경고) |
| 업데이트 | `tauri-plugin-updater` + GitHub Releases `latest.json` | 동일 |

- macOS 최소 버전은 **14**. 처음 이유였던 웹뷰 프록시는 없앴지만, View Transitions 등 최신 WebKit 기능을 쓰므로 그대로 둔다. 낮출 필요가 생기면 그때 기능별로 확인한다.
- 번들 ID 제안: `pro.tb.nyabi.Tailgate`
- 버전은 `src-tauri/tauri.conf.json` 한곳에서만 관리한다. 태그(`v0.1.0`)를 푸시하면 Actions가 두 플랫폼을 빌드해 GitHub Release 초안을 만든다.
- 업데이트 서명 키(`tauri signer generate`)의 개인 키는 GitHub Secrets에만 둔다. 잃어버리면 기존 설치본에 업데이트를 보낼 수 없으므로 따로 백업한다.
- 공증용 Apple 자격 증명(App Store Connect API 키)도 GitHub Secrets에 둔다.
- **배포 시 e621 UA**: 로그인한 사용자 이름을 넣고, 로그인하지 않았으면 개발자 연락처를 넣는다(`Tailgate/0.1 (by Nyabi on e621)`).
- 처음 실행할 때 18세 이상 확인 화면을 둔다(e621 약관).

## 9. 위험 요소

| 위험 | 대응 |
|---|---|
| 유저스페이스 터널 (gotatun 적합성, smoltcp 처리량) | **macOS에서 검증 완료**: 시스템 VPN 없이 e621 API 200, Cloudflare 10MB 5.2 MB/s(직접 9.4 MB/s, 경로가 도쿄 경유라 동일 조건 아님). Windows에서는 아직. |
| 동영상이 `media://`로 사실상 재생 안 됨 | 원인 확인: WKWebView가 MP4를 수~수십 바이트 Range로 수백 번 읽고, 각각이 도쿄 왕복(약 145ms)이 된다. 블록 캐시로 해결(13절). |
| WebView2의 커스텀 프로토콜 동영상 Range 처리 | Windows에서 실측 필요. 주소 형식은 `http://media.localhost/…`. |
| 루프백 SOCKS 포트를 다른 로컬 프로세스가 쓸 수 있음 | 127.0.0.1에만 바인드, 실행마다 임의 포트, 앱 종료 시 닫힘. 이제 웹뷰는 쓰지 않으므로, 필요하면 reqwest 사용자 정의 커넥터로 바꿔 포트 자체를 없앨 수 있다. |
| 터널 시작 지연 (약 3~4초, 대부분 600KB 서버 목록 받기) | 서버 목록을 디스크에 캐시하고 백그라운드에서 갱신한다(1단계 9번). |
| 터널 끊김 시 트래픽 누출 | 킬 스위치 기본(구현됨). 터널 모드에서 직접 연결로 넘어가는 코드 경로가 없다. 실행 중 끊김 감지와 재연결은 아직이다. |
| Mullvad API 변경, 기기 수 제한 | Mullvad 코드는 `tunnel/mullvad.rs` 한곳. 기기 수 초과 응답은 "다른 기기를 지우세요" 안내로 처리. |
| macOS WKWebView의 WebM 재생 여부 미확인 | MP4 변형을 먼저 쓴다. MP4 없는 WebM은 실측 후 대체 방법을 정한다. |
| 수천 장 스크롤 시 메모리·DOM 증가 | 보이는 행만 그리는 가상 스크롤. 그리드에서는 동영상을 재생하지 않는다. |
| UA·rate limit 위반으로 차단 | UA와 스로틀은 `api/client.rs` 한곳에서만. 그 밖의 API 호출 경로를 만들지 않는다. |
| legacy 응답 형식 변경 | 필수가 아닌 필드는 `Option`. 픽스처로 디코딩 테스트 고정. |
| TypeScript 7과 svelte-check 호환 | 확인함: svelte-check 4.7.7이 TS 7을 지원하지 않아 **6.0.3에 고정**. svelte-check가 지원하면 올린다. |
| 비밀 값 노출 (e621 API 키, WireGuard 개인 키, Mullvad 계정 번호) | OS 키체인에만 저장. 웹뷰로 보내지 않음. 로그·오류 메시지에 넣지 않음. |
| 업데이트 서명 키 분실 | 별도 백업. 분실 시 사용자가 수동으로 재설치해야 한다. |

## 10. 결정 사항과 남은 확인

정해짐 (2026-10-19):
- MVP는 2절 1단계 구성대로 간다.
- **기본 언어는 영어**, 영어·한국어를 먼저 지원한다(12절).
- GitHub **공개 저장소** `Tailgate`. 자동 업데이트는 그 저장소의 Releases를 쓴다.
- 개발자 e621 사용자명은 `Nyabi`. 로그인 전 UA는 `Tailgate/<버전> (by Nyabi on e621)`.

- 스타일은 **Tailwind CSS v4** (사용자 요청).
- 미디어는 **Rust 경유 `media://`** (static1 핫링크 차단 때문, 사용자 승인).
- 터널 구현은 GPL `wireguard-netstack` 대신 **gotatun 0.10 + smoltcp 0.14로 직접 조립** (오래된 gotatun 0.2에 묶여 있고, 배포 바이너리가 GPL이 되기 때문).

남은 것 (해당 단계 전까지만 있으면 된다):
- Windows 테스트 기기나 VM — 터널·`media://`를 Windows에서 실측할 때, 그리고 1단계 11번
- Developer ID Application 인증서 — 1단계 11번 전 (키체인에 아직 없음)

## 11. 단계별 진행

각 항목이 끝나면 확인할 수 있게 알리고, 요청하면 커밋한다.

**1단계**
1. 골격: `git init`, Tauri 2 + Svelte 5 + Vite, 번들 ID, 글꼴 번들, Tailwind 토큰, macOS 투명 제목줄. 빈 창 확인. (완료)
2. **터널 개념 검증**: `.conf` 파서 → gotatun + smoltcp → SOCKS5 → reqwest. 미디어는 `media://`로 Rust 경유. (진행 중, 13절)
3. API 계층: 클라이언트·스로틀·모델, Pawru 픽스처로 디코딩 테스트.
4. 계정: keyring, 로그인 화면, `/users/me`.
5. 검색: 태그 입력·자동완성, justified 그리드·가상 스크롤·무한 스크롤.
6. 블랙리스트: 테스트 먼저, 그리드 연결.
7. 뷰어: 전환 애니메이션, 줌·패닝, 원본 로드, 동영상, 태그 패널, 키보드.
8. 즐겨찾기·투표·다운로드.
9. 연결 설정 완성: Mullvad 로그인·서버 선택, 킬 스위치, 상태 표시.
10. 설정, 18세 확인 화면.
11. 배포: GitHub Actions(두 플랫폼), macOS 서명·공증, 업데이터.

**2단계** 태그 동작·탭 → Pool → Set → 댓글/DText → 인기

**3단계** 필요한 것부터.

## 12. 다국어

- **기본 언어는 영어.** 시스템 언어가 한국어면 한국어로 시작한다. 설정에서 "시스템 / English / 한국어"를 고를 수 있다(1단계 10번).
- 라이브러리 없이 `src/lib/i18n/`에 언어별 사전을 둔다. `en.ts`가 기준이고 `ko.ts`는 같은 타입(`Messages`)을 따라야 하므로 **키가 빠지면 타입 검사에서 실패한다.**
- 화면에서는 `m().nav.search`처럼 객체 경로로 읽는다. 문자열 키를 쓰지 않으므로 오타가 컴파일 단계에서 걸린다.
- 숫자에 따라 형태가 바뀌는 문구(복수형 등)는 문자열 대신 함수로 정의한다. 예: `hidden: (n) => n === 1 ? "1 post hidden" : `${n} posts hidden``.
- 날짜·숫자는 `Intl`에 현재 언어를 넘겨 형식화한다.
- **Rust는 사용자에게 보일 문구를 만들지 않는다.** 오류는 `{ code: "unauthorized" }`처럼 코드와 값만 보내고, 프런트엔드가 언어별 문구로 바꾼다.
- 코드 주석과 개발자용 오류 메시지는 영어로 쓴다(공개 저장소).
- Windows 설치 파일(NSIS)도 영어·한국어를 넣는다. macOS 앱 메뉴는 Tauri 기본 메뉴를 쓰며, 메뉴를 직접 만들게 되면 같은 사전을 쓴다.
- 언어를 더 늘릴 때는 사전 파일 하나를 추가하고 `Locale` 타입에 넣으면 된다.

## 13. 진행 상황 (2026-10-19 세션 끝)

### 된 것
- **골격과 UI 껍데기**: 반투명 사이드바, 떠 있는 본문 패널, 검색창(⌘K·Ctrl K·`/`), 빈 검색 화면과 예시 태그, 영어·한국어. 앱 아이콘(`src-tauri/icons/source.svg`가 원본).
- **앱 내장 터널 (macOS)**: Mullvad 기기 등록 → 도쿄 서버로 터널 → SOCKS5 → e621 API 200. 시스템 VPN을 끈 상태(e621 직접 접속은 차단됨)에서 확인했다.
- **연결 상태와 킬 스위치**: Rust → 화면 이벤트, 준비 전에는 원격 요청 차단.
- **`media://`**: 이미지가 터널 경유로 정상 표시된다.
- 테스트 8개(.conf 파서 5, media 경로·Range 3), `svelte-check`·빌드 통과.

### 다음 세션에서 바로 할 것
1. **동영상 블록 캐시** (`media.rs`): 상위 요청은 1MB 정렬 블록으로 받아 메모리에 두고(전체 256MB 한도, 오래된 것부터 버림), 작은 Range는 캐시에서 응답한다. 같은 블록 동시 요청은 한 번만 받고(`tokio::sync::OnceCell`), 블록 끝까지 읽히면 다음 블록을 미리 받는다. 응답은 한 블록 안으로 잘라 206으로 준다. Range 없는 요청(이미지)은 지금처럼 통째로 받는다. 이게 되면 지금의 `cap_range`는 필요 없어진다.
2. 동영상 재생과 탐색 반응성 확인. 확인용 임시 패널은 커밋하지 않았다. 다시 확인할 때는 `canLoadRemote()`가 참일 때만 `mediaUrl(...)`로 `<img>`/`<video>`를 띄우면 된다(예: 게시물 6791442의 720p, 6791519의 원본 PNG).
3. 그다음 1단계 3번(API 계층)으로 넘어간다.

### 개발 메모
- 터널 수동 점검: `cd src-tauri && cargo run --example tunnel_probe` (`--forget`은 Mullvad 기기 등록 해제, `PROBE_DOWNLOAD=<url>`은 속도 측정).
- Mullvad 계정 번호는 사용자가 직접 넣는다: `security add-generic-password -s pro.tb.nyabi.Tailgate -a mullvad-account -w`. 이 계정에는 지금 `divine condor` 기기가 등록되어 있다.
- 디버그 로그: `RUST_LOG=info,tailgate_lib=debug npm run tauri dev`. 웹 인스펙터는 `TAILGATE_DEVTOOLS=1`(디버그 빌드에서만).
- 화면 캡처 검증은 터미널에 화면 기록 권한이 있어야 한다. 키 입력 자동화(손쉬운 사용) 권한은 없다.
