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
    signInAgain: "Sign in again",
    title: "Sign in to e621",
    body: "Use your e621 username and an API key. The key is kept in the system's credential store.",
    username: "Username",
    apiKey: "API key",
    showKey: "Show API key",
    hideKey: "Hide API key",
    getKey: "Create an API key on e621",
    submit: "Sign in",
    submitting: "Signing in…",
    cancel: "Cancel",
    close: "Close",
    signedInAs: (name: string) => `Signed in to e621 as ${name}.`,
    signOut: "Sign out",
    rejected: (name: string) => `e621 no longer accepts the saved API key for ${name}. It may have expired.`,
    failure: {
      credentialStore: "Could not read the saved account.",
      savedAccount: "The saved account is damaged.",
    },
    error: {
      unauthorized: "The username or API key is wrong.",
      network: "Could not reach e621. Check the connection.",
      rateLimited: "Too many requests. Try again in a moment.",
      unavailable: "e621 is not answering right now. Try again later.",
      credentialStore: "Could not use the system's credential store.",
      other: "Something went wrong. Try again.",
    },
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
