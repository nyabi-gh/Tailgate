import { invoke } from "@tauri-apps/api/core";
import { attempt, type SettingsChangeError } from "./connection.svelte";
import { useLanguage, type Language } from "./i18n/index.svelte";

/** Mirrors `General` in src-tauri/src/settings.rs. */
export type GeneralSettings = { language: Language; adult: boolean };

let general = $state<GeneralSettings>({ language: "system", adult: false });

export function generalSettings(): GeneralSettings {
  return general;
}

/** Read before the first render, so the page starts in the chosen language and behind the age check when it should. */
export async function loadGeneralSettings() {
  general = await invoke<GeneralSettings>("general_settings");
  useLanguage(general.language);
}

/** Takes effect at once, also when it cannot be saved. */
export function changeGeneralSettings(change: Partial<GeneralSettings>): Promise<SettingsChangeError | null> {
  general = { ...general, ...change };
  useLanguage(general.language);
  return attempt(invoke("set_general_settings", { general }));
}

export function quit(): Promise<void> {
  return invoke("quit");
}
