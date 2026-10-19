const en = {
  nav: {
    search: "Search",
    favorites: "Favorites",
    popular: "Popular",
    pools: "Pools",
    sets: "Sets",
    settings: "Settings",
  },
  connection: {
    direct: "Direct connection",
    connecting: "Connecting…",
    tunnel: (relay: string) => `VPN · ${relay}`,
    failed: "VPN not connected",
    failure: {
      credentialStore: "Could not read the saved VPN settings.",
      savedDevice: "The saved VPN settings are damaged.",
      relayList: "Could not get the Mullvad server list.",
      noRelay: "No Mullvad server is available at the chosen location.",
      tunnel: "Could not start the VPN.",
      handshake: "The VPN server did not answer.",
    },
  },
  account: {
    signIn: "Sign in",
  },
  search: {
    placeholder: "Search tags",
    emptyTitle: "Search by tags",
    emptyBody: "Separate tags with spaces. Put a minus sign in front of a tag to leave it out.",
    examplesLabel: "Try one",
  },
};

export default en;

type Widen<T> = {
  [K in keyof T]: T[K] extends string
    ? string
    : T[K] extends (...args: infer A) => string
      ? (...args: A) => string
      : Widen<T[K]>;
};

export type Messages = Widen<typeof en>;
