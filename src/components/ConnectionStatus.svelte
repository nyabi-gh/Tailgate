<script lang="ts">
  import { CircleAlert } from "@lucide/svelte";
  import { connection } from "../lib/connection.svelte";
  import { m } from "../lib/i18n/index.svelte";

  const status = $derived(connection());
</script>

{#if status.state === "failed"}
  <CircleAlert size={16} strokeWidth={1.75} class="shrink-0" />
{:else}
  <span
    class={[
      "mx-[5px] size-1.5 shrink-0 rounded-full",
      status.state === "connecting" && "border border-faint",
      status.state === "direct" && "bg-faint",
      status.state === "tunnel" && "bg-focus",
    ]}
  ></span>
{/if}
<span class="truncate">
  {#if status.state === "direct"}
    {m().connection.direct}
  {:else if status.state === "connecting"}
    {m().connection.connecting}
  {:else if status.state === "tunnel"}
    {m().connection.tunnel(status.relay)}
  {:else}
    {m().connection.failed}
  {/if}
</span>
