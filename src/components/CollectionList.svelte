<script lang="ts">
  import { CircleAlert, ImageOff } from "@lucide/svelte";
  import { canLoadRemote } from "../lib/connection.svelte";
  import { apiErrorMessage } from "../lib/errors";
  import { m } from "../lib/i18n/index.svelte";
  import type { ListState } from "../lib/lists.svelte";
  import { mediaUrl } from "../lib/media";
  import type { ApiErrorCode } from "../lib/posts";

  /** A pool or a set as a row: its first post as the cover, and a line about it. */
  type Row = { id: number; name: string; detail: string; cover: string | null; note?: string };

  let {
    list,
    rows,
    empty,
    onopen,
  }: {
    list: ListState;
    rows: Row[];
    /** What to say when the list has nothing at all. */
    empty: string;
    onopen: (id: number) => void;
  } = $props();

  let scroller: HTMLDivElement;
  let content: HTMLDivElement;
  let scrollTop = $state(0);
  let viewport = $state(0);
  let height = $state(0);

  $effect(() => {
    const observer = new ResizeObserver(() => {
      viewport = scroller.clientHeight;
      height = scroller.scrollHeight;
    });
    observer.observe(scroller);
    observer.observe(content);
    return () => observer.disconnect();
  });

  // As the grids do: the next page while the end is still a screen and a half away.
  $effect(() => {
    if (viewport > 0 && height - (scrollTop + viewport) < 1.5 * viewport && canLoadRemote()) {
      list.loadMore();
    }
  });

  let failedCovers = $state<Set<string>>(new Set());
</script>

<div
  bind:this={scroller}
  class="h-full overflow-y-auto"
  onscroll={() => {
    scrollTop = scroller.scrollTop;
    height = scroller.scrollHeight;
  }}
>
  <div bind:this={content} class="flex min-h-full flex-col">
    {#if rows.length === 0}
      <div class="grid flex-1 place-items-center px-8 pb-[12vh] text-dim">
        {#if list.error !== null}
          {@render failure(list.error)}
        {:else if list.done}
          <p>{empty}</p>
        {:else if !canLoadRemote()}
          <p>{m().search.waiting}</p>
        {:else}
          <p>{m().search.loading}</p>
        {/if}
      </div>
    {:else}
      <ul class="flex flex-col p-2">
        {#each rows as row (row.id)}
          {@const cover = row.cover}
          <li>
            <button
              class="flex w-full items-center gap-3.5 rounded-md p-2 text-left transition-colors hover:bg-hover"
              onclick={() => onopen(row.id)}
            >
              <div class="grid h-16 w-12 shrink-0 place-items-center overflow-hidden rounded-sm bg-raised text-faint">
                {#if cover && !failedCovers.has(cover)}
                  <img
                    class="size-full object-cover"
                    src={mediaUrl(cover)}
                    alt=""
                    loading="lazy"
                    onerror={() => (failedCovers = new Set([...failedCovers, cover]))}
                  />
                {:else}
                  <ImageOff size={16} strokeWidth={1.75} />
                {/if}
              </div>
              <div class="flex min-w-0 flex-col gap-0.5">
                <span class="flex min-w-0 items-center gap-2">
                  <span class="truncate font-medium">{row.name}</span>
                  {#if row.note}
                    <span class="shrink-0 rounded-sm bg-hover px-1.5 text-xs text-dim">{row.note}</span>
                  {/if}
                </span>
                <span class="truncate text-dim">{row.detail}</span>
              </div>
            </button>
          </li>
        {/each}
      </ul>
      {#if list.error !== null}
        <div class="grid place-items-center pt-2 pb-10 text-dim">{@render failure(list.error)}</div>
      {:else if list.loading}
        <p class="pt-2 pb-10 text-center text-dim">{m().search.loading}</p>
      {/if}
    {/if}
  </div>
</div>

{#snippet failure(code: ApiErrorCode)}
  <div class="flex flex-col items-center gap-3 text-center">
    <p class="flex items-start gap-2 text-ink">
      <CircleAlert size={16} strokeWidth={1.75} class="mt-px shrink-0" />{apiErrorMessage(code)}
    </p>
    <button
      class="h-8 rounded-md bg-selected px-3.5 font-medium text-ink transition-colors hover:bg-hover"
      onclick={() => list.retry()}
    >
      {m().search.retry}
    </button>
  </div>
{/snippet}
