<script lang="ts">
  import { CircleAlert } from "@lucide/svelte";
  import { tick } from "svelte";
  import { canLoadRemote } from "../lib/connection.svelte";
  import { m } from "../lib/i18n/index.svelte";
  import { firstRowBelow, justify, ROW_HEIGHT } from "../lib/layout";
  import { isMac } from "../lib/platform";
  import type { ApiErrorCode } from "../lib/posts";
  import { loadMore, retry, search } from "../lib/search.svelte";
  import Thumb from "./Thumb.svelte";

  let { rowHeight = $bindable() }: { rowHeight: number } = $props();

  /** From here on thumbnails use the sample instead of the preview. */
  const LARGE_ROW_HEIGHT = 300;
  const GAP = 3;
  const PADDING = 8;

  let scroller: HTMLDivElement;
  let width = $state(0);
  let viewport = $state(0);
  let scrollTop = $state(0);

  const aspects = $derived(search.posts.map((post) => post.file.width / post.file.height));
  const layout = $derived(justify(aspects, width, rowHeight, GAP));
  /** One screen above and below stays rendered, so fast scrolling does not show empty space. */
  const visible = $derived(
    layout.rows.slice(
      firstRowBelow(layout.rows, scrollTop - PADDING - viewport),
      firstRowBelow(layout.rows, scrollTop - PADDING + 2 * viewport),
    ),
  );

  $effect(() => {
    const observer = new ResizeObserver(() => {
      width = scroller.clientWidth - 2 * PADDING;
      viewport = scroller.clientHeight;
    });
    observer.observe(scroller);
    return () => observer.disconnect();
  });

  // Ask for the next page while the end is still a screen and a half away.
  $effect(() => {
    const remaining = layout.height + PADDING - (scrollTop + viewport);
    if (viewport > 0 && remaining < 1.5 * viewport && canLoadRemote()) loadMore();
  });

  // Not `onwheel`: zooming must cancel the scroll, which a passive listener cannot.
  $effect(() => {
    const onwheel = (event: WheelEvent) => {
      // Ctrl also arrives with trackpad pinches in WebKit.
      if (!(event.ctrlKey || (isMac && event.metaKey))) return;
      event.preventDefault();
      resize(rowHeight - event.deltaY);
    };
    scroller.addEventListener("wheel", onwheel, { passive: false });
    return () => scroller.removeEventListener("wheel", onwheel);
  });

  /** Keeps the post at the top of the view in place while the rows change size. */
  export async function resize(next: number) {
    const anchorRow = layout.rows[firstRowBelow(layout.rows, scrollTop - PADDING)];
    const anchor = anchorRow?.tiles[0]?.index;
    rowHeight = Math.round(Math.min(ROW_HEIGHT.max, Math.max(ROW_HEIGHT.min, next)));
    if (anchor === undefined) return;
    await tick();
    const row = layout.rows.find((candidate) => candidate.tiles.some((tile) => tile.index === anchor));
    if (row !== undefined) scroller.scrollTop = row.y + PADDING;
  }

  function errorMessage(code: ApiErrorCode): string {
    const messages = m().search.error;
    switch (code) {
      case "unauthorized":
      case "network":
      case "rateLimited":
      case "unavailable":
      case "pageLimit":
        return messages[code];
      default:
        return messages.other;
    }
  }
</script>

<div bind:this={scroller} class="h-full overflow-y-auto" onscroll={() => (scrollTop = scroller.scrollTop)}>
  {#if search.posts.length === 0}
    <div class="grid h-full place-items-center px-8 pb-[12vh] text-dim">
      {#if search.error !== null}
        {@render failure(search.error)}
      {:else if search.done}
        <p>{m().search.noResults}</p>
      {:else if !canLoadRemote()}
        <p>{m().search.waiting}</p>
      {:else}
        <p>{m().search.loading}</p>
      {/if}
    </div>
  {:else}
    <div class="relative" style:height="{layout.height}px" style:margin="{PADDING}px">
      {#each visible as row (row.y)}
        {#each row.tiles as tile (tile.index)}
          {@const post = search.posts[tile.index]}
          {#if post !== undefined}
            <Thumb
              {post}
              x={tile.x}
              y={row.y}
              width={tile.width}
              height={row.height}
              large={rowHeight >= LARGE_ROW_HEIGHT}
            />
          {/if}
        {/each}
      {/each}
    </div>
    {#if search.error !== null}
      <div class="grid place-items-center pt-4 pb-10 text-dim">{@render failure(search.error)}</div>
    {:else if search.loading}
      <p class="pt-4 pb-10 text-center text-dim">{m().search.loading}</p>
    {/if}
  {/if}
</div>

{#snippet failure(code: ApiErrorCode)}
  <div class="flex flex-col items-center gap-3 text-center">
    <p class="flex items-start gap-2 text-ink">
      <CircleAlert size={16} strokeWidth={1.75} class="mt-px shrink-0" />{errorMessage(code)}
    </p>
    <button class="h-8 rounded-md bg-selected px-3.5 font-medium text-ink transition-colors hover:bg-hover" onclick={retry}>
      {m().search.retry}
    </button>
  </div>
{/snippet}
