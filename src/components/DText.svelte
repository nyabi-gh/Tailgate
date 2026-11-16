<script lang="ts">
  import { parseDText, type Block } from "../lib/dtext";
  import { m } from "../lib/i18n/index.svelte";
  import DTextInline from "./DTextInline.svelte";

  let { source, onsearch }: { source: string; onsearch: (tags: string[]) => void } = $props();

  const parsed = $derived(parseDText(source));

  const folded = "flex flex-col gap-2 rounded-sm bg-hover px-2 py-1";
  const summary = "cursor-default font-medium text-dim";
</script>

{#snippet render(blocks: Block[])}
  {#each blocks as block, index (index)}
    {#if block.type === "paragraph"}
      <p><DTextInline nodes={block.children} {onsearch} /></p>
    {:else if block.type === "heading"}
      <p class="font-strong"><DTextInline nodes={block.children} {onsearch} /></p>
    {:else if block.type === "list"}
      <ul class="flex list-disc flex-col gap-0.5 pl-4">
        {#each block.items as item, at (at)}
          <li style:margin-left="{(item.depth - 1) * 12}px"><DTextInline nodes={item.children} {onsearch} /></li>
        {/each}
      </ul>
    {:else if block.type === "quote"}
      <blockquote class="flex flex-col gap-2 border-l-2 border-seam pl-2.5 text-dim">
        {@render render(block.children)}
      </blockquote>
    {:else if block.type === "spoiler"}
      <details class={folded}>
        <summary class={summary}>{m().dtext.spoiler}</summary>
        {@render render(block.children)}
      </details>
    {:else if block.type === "section"}
      <details class={folded} open={block.expanded}>
        <summary class={summary}>{block.title ?? m().dtext.section}</summary>
        {@render render(block.children)}
      </details>
    {:else}
      <pre class="overflow-x-auto rounded-sm bg-hover p-2 font-mono text-sm">{block.text}</pre>
    {/if}
  {/each}
{/snippet}

<div class="flex flex-col gap-2 [overflow-wrap:anywhere] select-text">
  {@render render(parsed)}
</div>
