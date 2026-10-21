import { invoke } from "@tauri-apps/api/core";
import { attempt, type SettingsChangeError } from "./connection.svelte";
import { m, useLanguage, type Language } from "./i18n/index.svelte";
import { CELL_WIDTH, clampCellWidth } from "./layout";
import { notify } from "./toast.svelte";

/** Mirrors `General` in src-tauri/src/settings.rs. */
export type GeneralSettings = { language: Language; adult: boolean };
/** Mirrors `Browse` in src-tauri/src/settings.rs. */
type BrowseSettings = { cellWidth: number | null; recentSearches: string[] };

const RECENT_SEARCHES = 10;
/** The cell width changes with every wheel step; the file only needs where it settles. */
const CELL_WIDTH_SAVE_DELAY_MS = 500;

let general = $state<GeneralSettings>({ language: "system", adult: false });
let browse = $state.raw<BrowseSettings>({ cellWidth: null, recentSearches: [] });
let browseSave: ReturnType<typeof setTimeout> | undefined;

export function generalSettings(): GeneralSettings {
  return general;
}

/**
 * Read before the first render, so the page starts in the chosen language, behind the age check
 * when it should, and at the saved thumbnail size.
 */
export async function loadSettings() {
  [general, browse] = await Promise.all([
    invoke<GeneralSettings>("general_settings"),
    invoke<BrowseSettings>("browse_settings"),
  ]);
  useLanguage(general.language);
}

/** Takes effect at once, also when it cannot be saved. */
export function changeGeneralSettings(change: Partial<GeneralSettings>): Promise<SettingsChangeError | null> {
  general = { ...general, ...change };
  useLanguage(general.language);
  return attempt(invoke("set_general_settings", { general }));
}

export function cellWidth(): number {
  return clampCellWidth(browse.cellWidth ?? CELL_WIDTH.initial);
}

export function setCellWidth(width: number) {
  changeBrowse({ ...browse, cellWidth: clampCellWidth(width) }, CELL_WIDTH_SAVE_DELAY_MS);
}

/** Newest first. */
export function recentSearches(): string[] {
  return browse.recentSearches;
}

/** The search without tags is left out: it is the start screen, always one Enter away. */
export function rememberSearch(query: string) {
  if (query === "") return;
  const others = browse.recentSearches.filter((recent) => recent !== query);
  changeBrowse({ ...browse, recentSearches: [query, ...others].slice(0, RECENT_SEARCHES) });
}

export function clearRecentSearches() {
  changeBrowse({ ...browse, recentSearches: [] });
}

/** Takes effect at once; a failed save only shows a notice, since nothing waits on it. */
function changeBrowse(next: BrowseSettings, delay = 0) {
  browse = next;
  clearTimeout(browseSave);
  browseSave = setTimeout(async () => {
    if ((await attempt(invoke("set_browse_settings", { browse }))) !== null) notify(m().settings.error.settings);
  }, delay);
}

export function quit(): Promise<void> {
  return invoke("quit");
}
