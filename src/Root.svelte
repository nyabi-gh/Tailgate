<script lang="ts">
  import App from "./App.svelte";
  import AgeCheck from "./components/AgeCheck.svelte";
  import { generalSettings } from "./lib/settings.svelte";
</script>

<!-- The app starts loading posts as soon as it exists, so it waits for the age check. -->
{#if generalSettings().adult}
  <App />
{:else}
  <AgeCheck />
{/if}
