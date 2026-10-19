import { mount } from "svelte";
import "./styles/app.css";
import App from "./App.svelte";
import { watchAccount } from "./lib/account.svelte";
import { watchConnection } from "./lib/connection.svelte";
import { isMac, isTauri } from "./lib/platform";

// Only the macOS window is transparent (built in src-tauri/src/lib.rs); elsewhere the page paints its own background.
if (isMac && isTauri) {
  document.documentElement.dataset.material = "vibrant";
}

if (isTauri) {
  watchConnection().catch((error) => console.error("could not read the connection state", error));
  watchAccount().catch((error) => console.error("could not read the account state", error));
}

const target = document.getElementById("app");
if (!target) throw new Error("index.html is missing the #app element");
mount(App, { target });
