import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

/** Mirrors `FailureCode` in src-tauri/src/network.rs. */
export type ConnectionFailure =
  | "credentialStore"
  | "settings"
  | "savedDevice"
  | "noWireGuardConfig"
  | "savedConfig"
  | "mullvadSignedOut"
  | "relayList"
  | "noRelay"
  | "tunnel"
  | "handshake"
  | "connectionLost";

export type Connection =
  | { state: "direct" }
  | { state: "connecting" }
  | { state: "tunnel"; relay: string }
  | { state: "failed"; code: ConnectionFailure };

/** Mirrors `ConnectionMode` in src-tauri/src/settings.rs. */
export type ConnectionMode = "direct" | "wireGuard" | "mullvad";

/** Mirrors `ConnectionDetails` in src-tauri/src/network.rs. */
export type ConnectionDetails = {
  mode: ConnectionMode | null;
  location: string;
  wireguard: { fileName: string; endpoint: string } | null;
  mullvadDevice: string | null;
};

export type LocationChoice = { code: string; country: string; city: string };

/** Mirrors `SettingsChangeError` in src-tauri/src/network.rs. */
export type SettingsChangeError =
  | { code: "settings" }
  | { code: "credentialStore" }
  | { code: "network" }
  | { code: "invalidAccount" }
  | { code: "tooManyDevices" }
  | { code: "mullvad" }
  | { code: "unreadableFile" }
  | { code: "invalidConfig"; key: string; missing: boolean }
  | { code: "unknownLocation" };

const changeErrorCodes = [
  "settings",
  "credentialStore",
  "network",
  "invalidAccount",
  "tooManyDevices",
  "mullvad",
  "unreadableFile",
  "invalidConfig",
  "unknownLocation",
] as const;

let current = $state<Connection>({ state: "connecting" });
let routes = $state(0);

export function connection(): Connection {
  return current;
}

/** Remote media may only load once traffic has a route; before that the proxy refuses it. */
export function canLoadRemote(): boolean {
  return current.state === "direct" || current.state === "tunnel";
}

/**
 * Counts the routes opened so far. A change of route ends every connection of the old one, so
 * media that failed or was cut off then can be asked for again when this changes.
 */
export function routeCount(): number {
  return routes;
}

function update(next: Connection) {
  const wasRouted = canLoadRemote();
  current = next;
  if (!wasRouted && canLoadRemote()) routes += 1;
}

export async function watchConnection() {
  // Subscribe first so a change between the two calls is not missed. Events and command replies
  // arrive separately and in no fixed order, so an event that beats the reply is the newer state.
  let changed = false;
  await listen<Connection>("connection-changed", (event) => {
    changed = true;
    update(event.payload);
  });
  const initial = await invoke<Connection>("connection_status");
  if (!changed) update(initial);
}

export function connectionDetails(): Promise<ConnectionDetails> {
  return invoke<ConnectionDetails>("connection_details");
}

export function reconnect(): Promise<void> {
  return invoke("reconnect");
}

export function setConnectionMode(mode: ConnectionMode): Promise<SettingsChangeError | null> {
  return attempt(invoke("set_connection_mode", { mode }));
}

/** Resolves to `"cancelled"` when the user closes the file dialog without picking. */
export async function importWireGuardConfig(): Promise<SettingsChangeError | "cancelled" | null> {
  try {
    return (await invoke<boolean>("import_wireguard_config")) ? null : "cancelled";
  } catch (error) {
    if (isChangeError(error)) return error;
    throw error;
  }
}

export function mullvadSignIn(accountNumber: string): Promise<SettingsChangeError | null> {
  return attempt(invoke("mullvad_sign_in", { accountNumber }));
}

export function mullvadSignOut(): Promise<SettingsChangeError | null> {
  return attempt(invoke("mullvad_sign_out"));
}

/** When the Mullvad account is paid until, or `null` when it cannot be told. */
export async function mullvadExpiry(): Promise<Date | null> {
  try {
    const expiry = await invoke<string | null>("mullvad_expiry");
    return expiry === null ? null : new Date(expiry);
  } catch (error) {
    if (isChangeError(error)) return null;
    throw error;
  }
}

export function mullvadLocations(): Promise<LocationChoice[] | SettingsChangeError> {
  return invoke<LocationChoice[]>("mullvad_locations").catch((error: unknown) => {
    if (isChangeError(error)) return error;
    throw error;
  });
}

export function setMullvadLocation(code: string): Promise<SettingsChangeError | null> {
  return attempt(invoke("set_mullvad_location", { code }));
}

/** Resolves to the command's failure, or `null`. Anything else is a bug and is rethrown. */
export async function attempt(call: Promise<unknown>): Promise<SettingsChangeError | null> {
  try {
    await call;
    return null;
  } catch (error) {
    if (isChangeError(error)) return error;
    throw error;
  }
}

function isChangeError(error: unknown): error is SettingsChangeError {
  if (typeof error !== "object" || error === null || !("code" in error)) return false;
  const code = error.code;
  return changeErrorCodes.some((known) => known === code);
}
