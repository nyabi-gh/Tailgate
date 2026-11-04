import { createContext } from "svelte";

export type View = "search" | "favorites" | "popular" | "pools" | "sets" | "settings";

export type BrowseView = Exclude<View, "settings">;

export const browseViews: BrowseView[] = ["search", "favorites", "popular", "pools", "sets"];

/** Lets a screen deep in the app, such as a list waiting for the connection, open the settings. */
export const [getOpenSettings, setOpenSettings] = createContext<() => void>();
