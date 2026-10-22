import { getVersion } from "@tauri-apps/api/app";
import { Channel, invoke } from "@tauri-apps/api/core";
import { attempt } from "./connection.svelte";
import { m } from "./i18n/index.svelte";
import { notify } from "./toast.svelte";

/** Mirrors `Available` in src-tauri/src/update.rs. */
export type Available = { version: string; notes: string | null };
/** Mirrors `UpdateError` in src-tauri/src/update.rs. */
export type UpdateFailure = "network" | "invalidRelease" | "install";
/** Mirrors `Progress` in src-tauri/src/update.rs, in bytes. */
type Progress = { received: number; total: number | null };
/** Mirrors `UpdateSettings` in src-tauri/src/settings.rs. */
type UpdateSettings = { checkAutomatically: boolean };

export type UpdateState =
  | { phase: "idle" }
  | { phase: "checking" }
  | { phase: "upToDate" }
  | { phase: "checkFailed"; code: UpdateFailure }
  | { phase: "available"; update: Available }
  | { phase: "downloading"; update: Available; percent: number | null }
  /** macOS only: on Windows the installer closes the app. */
  | { phase: "installed"; update: Available }
  | { phase: "installFailed"; update: Available; code: UpdateFailure };

const failures = ["network", "invalidRelease", "install"] as const;

let state = $state<UpdateState>({ phase: "idle" });
let version = $state("");
let settings = $state<UpdateSettings>({ checkAutomatically: true });
/** Set when a check finds a version the user has not been shown yet; the sheet opens on it. */
let offered = $state<Available | null>(null);
let checkedThisRun = false;

getVersion().then((current) => (version = current));
invoke<UpdateSettings>("update_settings").then((saved) => (settings = saved));

export function updateState(): UpdateState {
  return state;
}

export function appVersion(): string {
  return version;
}

export function checksAutomatically(): boolean {
  return settings.checkAutomatically;
}

/** Takes effect at once; a failed save only shows a notice. */
export async function setChecksAutomatically(checkAutomatically: boolean) {
  settings = { checkAutomatically };
  if ((await attempt(invoke("set_update_settings", { updates: settings }))) !== null) {
    notify(m().settings.error.settings);
  }
}

export function offeredUpdate(): Available | null {
  return offered;
}

export function dismissOffer() {
  offered = null;
}

export function showUpdate() {
  if ("update" in state) offered = state.update;
}

function failure(error: unknown): UpdateFailure {
  if (typeof error === "object" && error !== null && "code" in error) {
    const code = error.code;
    const known = failures.find((failure) => failure === code);
    if (known) return known;
  }
  throw error;
}

export async function checkForUpdate() {
  if (state.phase === "checking" || state.phase === "downloading") return;
  checkedThisRun = true;
  state = { phase: "checking" };
  try {
    const update = await invoke<Available | null>("check_for_update");
    state = update === null ? { phase: "upToDate" } : { phase: "available", update };
    offered = update;
  } catch (error) {
    state = { phase: "checkFailed", code: failure(error) };
  }
}

/**
 * Once a run, when the connection first opens. A failure stays quiet: the settings screen shows
 * it, and the next run tries again.
 */
export async function checkOnceAutomatically() {
  if (checkedThisRun) return;
  const saved = await invoke<UpdateSettings>("update_settings");
  if (!saved.checkAutomatically || checkedThisRun) return;
  await checkForUpdate();
}

export async function installUpdate() {
  if (!("update" in state) || state.phase === "downloading") return;
  const update = state.update;
  state = { phase: "downloading", update, percent: null };
  const progress = new Channel<Progress>();
  progress.onmessage = ({ received, total }) => {
    if (state.phase !== "downloading") return;
    state = { ...state, percent: total ? Math.min(100, Math.floor((received / total) * 100)) : null };
  };
  try {
    await invoke("install_update", { progress });
    state = { phase: "installed", update };
  } catch (error) {
    state = { phase: "installFailed", update, code: failure(error) };
  }
}

export function restart(): Promise<void> {
  return invoke("restart");
}

/** Resolves to whether the notices opened. */
export async function openNotices(): Promise<boolean> {
  try {
    await invoke("open_notices");
    return true;
  } catch (error) {
    if (typeof error === "object" && error !== null && "code" in error) return false;
    throw error;
  }
}
