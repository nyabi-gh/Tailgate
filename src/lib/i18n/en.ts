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
