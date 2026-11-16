<script lang="ts">
  import { m } from "../lib/i18n/index.svelte";
  import DText from "./DText.svelte";

  let { source, onsearch }: { source: string; onsearch: (tags: string[]) => void } = $props();

  let box: HTMLElement | undefined = $state();
  let expanded = $state(false);
  let overflows = $state(false);

  $effect(() => {
    if (box && !expanded) overflows = box.scrollHeight > box.clientHeight;
  });
</script>

<section class="flex flex-col gap-1.5">
  <h3 class="text-xs text-faint">{m().viewer.description}</h3>
  <div bind:this={box} class={["overflow-hidden", !expanded && "max-h-40"]}>
    <DText {source} {onsearch} />
  </div>
  {#if overflows}
    <button class="w-fit text-focus hover:underline" onclick={() => (expanded = !expanded)}>
      {expanded ? m().viewer.showLess : m().viewer.showMore}
    </button>
  {/if}
</section>
