import type { Messages } from "./en";

const ko: Messages = {
  nav: {
    search: "검색",
    favorites: "즐겨찾기",
    popular: "인기",
    pools: "풀",
    sets: "세트",
    settings: "설정",
  },
  connection: {
    direct: "직접 연결",
    connecting: "연결 중…",
    tunnel: (relay) => `VPN · ${relay}`,
    failed: "VPN 연결 안 됨",
    failure: {
      credentialStore: "저장된 VPN 설정을 읽지 못했습니다.",
      savedDevice: "저장된 VPN 설정이 손상되었습니다.",
      relayList: "Mullvad 서버 목록을 받지 못했습니다.",
      noRelay: "고른 위치에 쓸 수 있는 Mullvad 서버가 없습니다.",
      tunnel: "VPN을 시작하지 못했습니다.",
      handshake: "VPN 서버가 응답하지 않습니다.",
    },
  },
  account: {
    signIn: "로그인",
  },
  search: {
    placeholder: "태그 검색",
    emptyTitle: "태그로 검색",
    emptyBody: "태그는 띄어쓰기로 구분합니다. 태그 앞에 -를 붙이면 그 태그가 붙은 게시물은 빠집니다.",
    examplesLabel: "예시",
  },
};

export default ko;
