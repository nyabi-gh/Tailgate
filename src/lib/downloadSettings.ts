import { invoke } from "@tauri-apps/api/core";
import { isChangeError, type SettingsChangeError } from "./connection.svelte";

/** Mirrors `DownloadDetails` in src-tauri/src/download.rs. */
export type DownloadDetails = { folder: string; name: string; example: string | null };

/** Mirrors `TemplateError` in src-tauri/src/download/name.rs. */
export type NameRuleError =
  | { code: "unknownToken"; token: string }
  | { code: "unclosed" }
  | { code: "notUnique" }
  | { code: "separator" };

const nameRuleCodes = ["unknownToken", "unclosed", "notUnique", "separator"] as const;

export function isNameRuleError(error: unknown): error is NameRuleError {
  if (typeof error !== "object" || error === null || !("code" in error)) return false;
  const code = error.code;
  return nameRuleCodes.some((known) => known === code);
}

export function downloadDetails(): Promise<DownloadDetails> {
  return invoke<DownloadDetails>("download_settings");
}

/** Resolves to `"cancelled"` when the user closes the folder dialog without picking. */
export async function chooseDownloadFolder(): Promise<SettingsChangeError | "cancelled" | null> {
  try {
    return (await invoke<boolean>("choose_download_folder")) ? null : "cancelled";
  } catch (error) {
    if (isChangeError(error)) return error;
    throw error;
  }
}

/** Resolves to `false` when the folder could not be created or opened. */
export function openDownloadFolder(): Promise<boolean> {
  return invoke("open_download_folder").then(
    () => true,
    (error: unknown) => {
      console.error("could not open the download folder", error);
      return false;
    },
  );
}

/** The name an example post would get, or why the rule cannot be used. */
export async function exampleName(name: string): Promise<{ example: string } | { error: NameRuleError }> {
  try {
    return { example: await invoke<string>("download_name_example", { name }) };
  } catch (error) {
    if (isNameRuleError(error)) return { error };
    throw error;
  }
}

export async function setDownloadName(name: string): Promise<NameRuleError | SettingsChangeError | null> {
  try {
    await invoke("set_download_name", { name });
    return null;
  } catch (error) {
    if (isNameRuleError(error) || isChangeError(error)) return error;
    throw error;
  }
}
