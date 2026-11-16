<script lang="ts">
  import type { Snippet } from "svelte";
  import { m } from "../lib/i18n/index.svelte";

  let { children }: { children: Snippet } = $props();

  let shown = $state(false);
</script>

{#if shown}
  <span class="rounded-sm bg-hover">{@render children()}</span>
{:else}
  <button class="rounded-sm bg-hover px-1.5 text-dim hover:text-ink" onclick={() => (shown = true)}>
    {m().dtext.spoiler}
  </button>
{/if}
