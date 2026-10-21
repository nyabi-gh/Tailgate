import { mount } from "svelte";
import "./styles/app.css";
import Root from "./Root.svelte";
import { watchAccount } from "./lib/account.svelte";
import { watchDownloads } from "./lib/actions.svelte";
import { watchConnection } from "./lib/connection.svelte";
import { isMac, isTauri } from "./lib/platform";
import { watchBlacklist } from "./lib/search.svelte";
import { loadSettings } from "./lib/settings.svelte";

// Only the macOS window is transparent (built in src-tauri/src/lib.rs); elsewhere the page paints its own background.
if (isMac && isTauri) {
  document.documentElement.dataset.material = "vibrant";
}

if (isTauri) {
  watchConnection().catch((error) => console.error("could not read the connection state", error));
  watchAccount().catch((error) => console.error("could not read the account state", error));
  watchDownloads().catch((error) => console.error("could not follow downloads", error));
  watchBlacklist().catch((error) => console.error("could not follow blacklist changes", error));
}

const target = document.getElementById("app");
if (!target) throw new Error("index.html is missing the #app element");
const settingsRead = isTauri
  ? loadSettings().catch((error) => console.error("could not read the settings", error))
  : Promise.resolve();
settingsRead.then(() => mount(Root, { target }));
