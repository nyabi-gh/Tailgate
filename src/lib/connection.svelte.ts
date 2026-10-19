import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

/** Mirrors `FailureCode` in src-tauri/src/network.rs. */
export type ConnectionFailure = "credentialStore" | "savedDevice" | "relayList" | "noRelay" | "tunnel";

export type Connection =
  | { state: "direct" }
  | { state: "connecting" }
  | { state: "tunnel"; relay: string }
  | { state: "failed"; code: ConnectionFailure };

let current = $state<Connection>({ state: "connecting" });

export function connection(): Connection {
  return current;
}

/** Remote media may only load once traffic has a route; before that the proxy refuses it. */
export function canLoadRemote(): boolean {
  return current.state === "direct" || current.state === "tunnel";
}

export async function watchConnection() {
  // Subscribe first so a change between the two calls is not missed.
  await listen<Connection>("connection-changed", (event) => (current = event.payload));
  current = await invoke<Connection>("connection_status");
}
