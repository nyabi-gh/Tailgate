export type View = "search" | "favorites" | "popular" | "pools" | "sets" | "settings";

export type BrowseView = Exclude<View, "settings">;

export const browseViews: BrowseView[] = ["search", "favorites", "popular", "pools", "sets"];
