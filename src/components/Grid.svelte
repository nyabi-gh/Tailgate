<script lang="ts">
  import { CircleAlert, Eye, EyeOff } from "@lucide/svelte";
  import { tick } from "svelte";
  import { canLoadRemote } from "../lib/connection.svelte";
  import { apiErrorMessage } from "../lib/errors";
  import { m } from "../lib/i18n/index.svelte";
  import { cellPosition, clampCellWidth, grid, rowOf, visibleRange } from "../lib/layout";
  import { isMac } from "../lib/platform";
  import type { ApiErrorCode, Post } from "../lib/posts";
  import type { PostList } from "../lib/search.svelte";
  import Thumb from "./Thumb.svelte";

  let {
    list,
    cellWidth = $bindable(),
    transitionId,
    empty,
    onopen,
  }: {
    list: PostList;
    cellWidth: number;
    transitionId: number | null;
    /** What to say when the list has nothing at all. */
    empty: string;
    onopen: (post: Post) => void;
  } = $props();

  /** From this cell width on, thumbnails use the sample instead of the preview. */
  const LARGE_CELL_WIDTH = 240;
  const GAP = 3;
  const PADDING = 8;

  let scroller: HTMLDivElement;
  let width = $state(0);
  let viewport = $state(0);
  let scrollTop = $state(0);

  const layout = $derived(grid(list.posts.length, width, cellWidth, GAP));
  /** One screen above and below stays rendered, so fast scrolling does not show empty space. */
  const visible = $derived.by(() => {
    const [start, end] = visibleRange(
      layout,
      list.posts.length,
      scrollTop - PADDING - viewport,
      scrollTop - PADDING + 2 * viewport,
    );
    return list.posts.slice(start, end).map((post, offset) => ({ post, index: start + offset }));
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
    if (viewport > 0 && remaining < 1.5 * viewport && canLoadRemote()) list.loadMore();
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
  /** Scrolls the post into view if it is not, and waits until its thumbnail is rendered. */
  export async function reveal(id: number) {
    const index = list.posts.findIndex((post) => post.id === id);
    if (index < 0) return;
    const pitch = layout.cellHeight + layout.gap;
    const top = rowOf(layout, index) * pitch + PADDING;
    if (top < scroller.scrollTop || top + layout.cellHeight > scroller.scrollTop + viewport) {
      scroller.scrollTop = top - (viewport - layout.cellHeight) / 2;
      scrollTop = scroller.scrollTop;
    }
    await tick();
  }

  export async function resize(next: number) {
    const [anchor] = visibleRange(layout, list.posts.length, scrollTop - PADDING, scrollTop - PADDING);
    cellWidth = clampCellWidth(next);
    if (anchor >= list.posts.length) return;
    await tick();
    scroller.scrollTop = rowOf(layout, anchor) * (layout.cellHeight + layout.gap) + PADDING;
  }
</script>

<div class="relative h-full">
<div bind:this={scroller} class="h-full overflow-y-auto" onscroll={() => (scrollTop = scroller.scrollTop)}>
  {#if list.posts.length === 0}
    <div class="grid h-full place-items-center px-8 pb-[12vh] text-dim">
      {#if list.error !== null}
        {@render failure(list.error)}
      {:else if list.done && list.hiddenCount > 0}
        <p>{m().search.allHidden}</p>
      {:else if list.done}
        <p>{empty}</p>
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
          blacklisted={list.isBlacklisted(post)}
          transitionName={transitionId === post.id ? "post-media" : undefined}
          onopen={() => onopen(post)}
        />
      {/each}
    </div>
    {#if list.error !== null}
      <div class="grid place-items-center pt-4 pb-10 text-dim">{@render failure(list.error)}</div>
    {:else if list.loading}
      <p class="pt-4 pb-10 text-center text-dim">{m().search.loading}</p>
    {/if}
  {/if}
</div>

{#if list.hiddenCount > 0}
  <button
    class="absolute right-4 bottom-4 flex h-8 items-center gap-1.5 rounded-full bg-panel px-3.5 font-medium text-ink shadow-panel transition-colors hover:bg-raised"
    title={list.revealed ? undefined : m().search.revealHint}
    aria-pressed={list.revealed}
    onclick={() => list.toggleRevealed()}
  >
    {#if list.revealed}
      <EyeOff size={15} strokeWidth={1.75} class="text-dim" />{m().search.hideAgain}
    {:else}
      <Eye size={15} strokeWidth={1.75} class="text-dim" />{m().search.hidden(list.hiddenCount)}
    {/if}
  </button>
{/if}
</div>

{#snippet failure(code: ApiErrorCode)}
  <div class="flex flex-col items-center gap-3 text-center">
    <p class="flex items-start gap-2 text-ink">
      <CircleAlert size={16} strokeWidth={1.75} class="mt-px shrink-0" />{apiErrorMessage(code)}
    </p>
    <button class="h-8 rounded-md bg-selected px-3.5 font-medium text-ink transition-colors hover:bg-hover" onclick={() => list.retry()}>
      {m().search.retry}
    </button>
  </div>
{/snippet}
