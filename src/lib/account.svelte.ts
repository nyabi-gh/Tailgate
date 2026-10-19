import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

/** Mirrors `AccountState` in src-tauri/src/account.rs. */
export type Account =
  | { state: "anonymous" }
  | { state: "signedIn"; name: string; id: number | null }
  | { state: "rejected"; name: string }
  | { state: "failed"; code: "credentialStore" | "savedAccount" };

/** Mirrors `AccountError` in src-tauri/src/account.rs: the API's error codes plus `credentialStore`. */
const signInErrors = [
  "unauthorized",
  "forbidden",
  "notFound",
  "pageLimit",
  "rateLimited",
  "unavailable",
  "network",
  "decode",
  "http",
  "credentialStore",
] as const;

export type SignInError = (typeof signInErrors)[number];

let current = $state<Account>({ state: "anonymous" });

export function account(): Account {
  return current;
}

export async function watchAccount() {
  // Same ordering as watchConnection: an event that beats the command reply is the newer state.
  let changed = false;
  await listen<Account>("account-changed", (event) => {
    changed = true;
    current = event.payload;
  });
  const initial = await invoke<Account>("account_status");
  if (!changed) current = initial;
}

/** Resolves to the failure code, or `null` once signed in. */
export async function signIn(username: string, apiKey: string): Promise<SignInError | null> {
  try {
    await invoke("sign_in", { username, apiKey });
    return null;
  } catch (error) {
    return errorCode(error);
  }
}

export async function signOut(): Promise<SignInError | null> {
  try {
    await invoke("sign_out");
    return null;
  } catch (error) {
    return errorCode(error);
  }
}

/** Commands reject with `{ code }`; anything else is a bug and is rethrown. */
function errorCode(error: unknown): SignInError {
  const code = typeof error === "object" && error !== null && "code" in error ? error.code : undefined;
  const known = signInErrors.find((candidate) => candidate === code);
  if (known === undefined) throw error;
  return known;
}
