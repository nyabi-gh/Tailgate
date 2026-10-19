<script lang="ts">
  import { m } from "../lib/i18n/index.svelte";

  let { onpick }: { onpick: (tag: string) => void } = $props();

  // Full class strings so Tailwind can find them when scanning the source.
  const examples = [
    { tag: "wolf", tone: "text-tag-species bg-tag-species/12 hover:bg-tag-species/20" },
    { tag: "dragon", tone: "text-tag-species bg-tag-species/12 hover:bg-tag-species/20" },
    { tag: "landscape", tone: "text-tag-general bg-tag-general/12 hover:bg-tag-general/20" },
    { tag: "rating:s", tone: "text-tag-meta bg-tag-meta/12 hover:bg-tag-meta/20" },
    { tag: "order:score", tone: "text-tag-meta bg-tag-meta/12 hover:bg-tag-meta/20" },
  ];
</script>

<div class="mx-auto flex h-full max-w-[520px] flex-col justify-center px-8 pb-[12vh]">
  <h1 class="text-xl font-strong tracking-[-0.015em]">{m().search.emptyTitle}</h1>
  <p class="mt-2 text-base text-dim">{m().search.emptyBody}</p>

  <p class="mt-7 text-xs text-faint">{m().search.examplesLabel}</p>
  <div class="mt-2 flex flex-wrap gap-1.5">
    {#each examples as example (example.tag)}
      <button
        class={["h-7 rounded-full px-3 text-sm font-medium transition-colors", example.tone]}
        onclick={() => onpick(example.tag)}
      >
        {example.tag}
      </button>
    {/each}
  </div>
</div>
