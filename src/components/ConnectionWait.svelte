<script lang="ts">
  import { CircleAlert } from "@lucide/svelte";
  import { connection, reconnect } from "../lib/connection.svelte";
  import { m } from "../lib/i18n/index.svelte";
  import { getOpenSettings } from "../lib/views";

  const status = $derived(connection());
  const openSettings = getOpenSettings();
</script>

<!-- Transient failures are retried by Rust on its own; the list loads once there is a route. -->
{#if status.state === "failed"}
  <div class="flex flex-col items-center gap-3 text-center">
    <p class="flex items-start gap-2 text-ink" role="alert">
      <CircleAlert size={16} strokeWidth={1.75} class="mt-px shrink-0" />{m().connection.failure[status.code]}
    </p>
    <div class="flex gap-2">
      <button
        class="h-8 rounded-md bg-selected px-3.5 font-medium text-ink transition-colors hover:bg-hover"
        onclick={reconnect}
      >
        {m().settings.reconnect}
      </button>
      <button class="h-8 rounded-md px-3 text-ink/80 transition-colors hover:bg-hover" onclick={openSettings}>
        {m().connection.openSettings}
      </button>
    </div>
  </div>
{:else}
  <p>{m().search.waiting}</p>
{/if}
