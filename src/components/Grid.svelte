<script lang="ts">
  import { CircleAlert } from "@lucide/svelte";
  import { tick } from "svelte";
  import { canLoadRemote } from "../lib/connection.svelte";
  import { m } from "../lib/i18n/index.svelte";
  import { CELL_WIDTH, cellPosition, grid, rowOf, visibleRange } from "../lib/layout";
  import { isMac } from "../lib/platform";
  import type { ApiErrorCode } from "../lib/posts";
  import { loadMore, retry, search } from "../lib/search.svelte";
  import Thumb from "./Thumb.svelte";

  let { cellWidth = $bindable() }: { cellWidth: number } = $props();

  /** From this cell width on, thumbnails use the sample instead of the preview. */
  const LARGE_CELL_WIDTH = 240;
  const GAP = 3;
  const PADDING = 8;

  let scroller: HTMLDivElement;
  let width = $state(0);
  let viewport = $state(0);
  let scrollTop = $state(0);

  const layout = $derived(grid(search.posts.length, width, cellWidth, GAP));
  /** One screen above and below stays rendered, so fast scrolling does not show empty space. */
  const visible = $derived.by(() => {
    const [start, end] = visibleRange(
      layout,
      search.posts.length,
      scrollTop - PADDING - viewport,
      scrollTop - PADDING + 2 * viewport,
    );
    return search.posts.slice(start, end).map((post, offset) => ({ post, index: start + offset }));
  });

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
      resize(cellWidth - event.deltaY);
    };
    scroller.addEventListener("wheel", onwheel, { passive: false });
    return () => scroller.removeEventListener("wheel", onwheel);
  });

  /** Keeps the post at the top of the view in place while the cells change size. */
  export async function resize(next: number) {
    const [anchor] = visibleRange(layout, search.posts.length, scrollTop - PADDING, scrollTop - PADDING);
    cellWidth = Math.round(Math.min(CELL_WIDTH.max, Math.max(CELL_WIDTH.min, next)));
    if (anchor >= search.posts.length) return;
    await tick();
    scroller.scrollTop = rowOf(layout, anchor) * (layout.cellHeight + layout.gap) + PADDING;
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
      {#each visible as { post, index } (post.id)}
        {@const position = cellPosition(layout, index)}
        <Thumb
          {post}
          x={position.x}
          y={position.y}
          width={layout.cellWidth}
          height={layout.cellHeight}
          large={cellWidth >= LARGE_CELL_WIDTH}
        />
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
