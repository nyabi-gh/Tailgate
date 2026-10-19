import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

/** Mirrors `FailureCode` in src-tauri/src/network.rs. */
export type ConnectionFailure = "credentialStore" | "savedDevice" | "relayList" | "noRelay" | "tunnel" | "handshake";

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
  // Subscribe first so a change between the two calls is not missed. Events and command replies
  // arrive separately and in no fixed order, so an event that beats the reply is the newer state.
  let changed = false;
  await listen<Connection>("connection-changed", (event) => {
    changed = true;
    current = event.payload;
  });
  const initial = await invoke<Connection>("connection_status");
  if (!changed) current = initial;
}
