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
    signInAgain: "다시 로그인",
    title: "e621 로그인",
    body: "e621 사용자명과 API 키로 로그인합니다. 키는 시스템 자격 증명 저장소에 보관됩니다.",
    username: "사용자명",
    apiKey: "API 키",
    showKey: "API 키 보기",
    hideKey: "API 키 가리기",
    getKey: "e621에서 API 키 만들기",
    submit: "로그인",
    submitting: "로그인 중…",
    cancel: "취소",
    close: "닫기",
    signedInAs: (name) => `e621에 ${name}(으)로 로그인되어 있습니다.`,
    signOut: "로그아웃",
    rejected: (name) => `e621이 ${name}의 저장된 API 키를 더 이상 받지 않습니다. 만료되었을 수 있습니다.`,
    failure: {
      credentialStore: "저장된 계정을 읽지 못했습니다.",
      savedAccount: "저장된 계정이 손상되었습니다.",
    },
    error: {
      unauthorized: "사용자명이나 API 키가 틀렸습니다.",
      network: "e621에 접속하지 못했습니다. 연결을 확인하세요.",
      rateLimited: "요청이 너무 많습니다. 잠시 뒤에 다시 시도하세요.",
      unavailable: "e621이 지금 응답하지 않습니다. 나중에 다시 시도하세요.",
      credentialStore: "시스템 자격 증명 저장소를 쓰지 못했습니다.",
      other: "문제가 생겼습니다. 다시 시도하세요.",
    },
  },
  search: {
    placeholder: "태그 검색",
    emptyTitle: "태그로 검색",
    emptyBody: "태그는 띄어쓰기로 구분합니다. 태그 앞에 -를 붙이면 그 태그가 붙은 게시물은 빠집니다.",
    examplesLabel: "예시",
  },
};

export default ko;
