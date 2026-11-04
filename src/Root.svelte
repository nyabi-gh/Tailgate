<script lang="ts">
  import App from "./App.svelte";
  import AgeCheck from "./components/AgeCheck.svelte";
  import { isMac } from "./lib/platform";
  import { generalSettings, resetPageZoom, zoomPageIn, zoomPageOut } from "./lib/settings.svelte";
  import { dispatchKey, onKeys } from "./lib/shortcuts";

  // The physical keys as well, since `key` follows the input source; the webview's own zoom keys
  // are off, so these are the only ones.
  onKeys("global", (event) => {
    if (!(isMac ? event.metaKey : event.ctrlKey) || event.altKey) return false;
    if (["Equal", "NumpadAdd"].includes(event.code) || event.key === "+") zoomPageIn();
    else if (["Minus", "NumpadSubtract"].includes(event.code) || event.key === "-") zoomPageOut();
    else if (["Digit0", "Numpad0"].includes(event.code)) resetPageZoom();
    else return false;
    event.preventDefault();
    return true;
  });
</script>

<svelte:window onkeydown={dispatchKey} />

<!-- The app starts loading posts as soon as it exists, so it waits for the age check. -->
{#if generalSettings().adult}
  <App />
{:else}
  <AgeCheck />
{/if}
