<script lang="ts">
  import { CircleAlert, ImageOff } from "@lucide/svelte";
  import { canLoadRemote } from "../lib/connection.svelte";
  import { apiErrorMessage } from "../lib/errors";
  import { currentLocale, m } from "../lib/i18n/index.svelte";
  import { mediaUrl } from "../lib/media";
  import { poolIndex, type PoolSummary } from "../lib/pools.svelte";
  import type { ApiErrorCode } from "../lib/posts";

  let { onopen }: { onopen: (pool: PoolSummary) => void } = $props();

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
      poolIndex.loadMore();
    }
  });

  const dateFormat = $derived(new Intl.DateTimeFormat(currentLocale(), { dateStyle: "medium" }));
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
    {#if poolIndex.pools.length === 0}
      <div class="grid flex-1 place-items-center px-8 pb-[12vh] text-dim">
        {#if poolIndex.error !== null}
          {@render failure(poolIndex.error)}
        {:else if poolIndex.done}
          <p>{m().pools.empty}</p>
        {:else if !canLoadRemote()}
          <p>{m().search.waiting}</p>
        {:else}
          <p>{m().search.loading}</p>
        {/if}
      </div>
    {:else}
      <ul class="flex flex-col p-2">
        {#each poolIndex.pools as pool (pool.id)}
          {@const cover = pool.cover}
          <li>
            <button
              class="flex w-full items-center gap-3.5 rounded-md p-2 text-left transition-colors hover:bg-hover"
              onclick={() => onopen(pool)}
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
                <span class="truncate font-medium">{pool.name}</span>
                <span class="truncate text-dim">
                  {m().pools.category[pool.category]} · {m().pools.posts(pool.postCount)} · {dateFormat.format(
                    new Date(pool.updatedAt),
                  )}{#if pool.creator}{" · "}{m().pools.by(pool.creator)}{/if}
                </span>
              </div>
            </button>
          </li>
        {/each}
      </ul>
      {#if poolIndex.error !== null}
        <div class="grid place-items-center pt-2 pb-10 text-dim">{@render failure(poolIndex.error)}</div>
      {:else if poolIndex.loading}
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
      onclick={() => poolIndex.retry()}
    >
      {m().search.retry}
    </button>
  </div>
{/snippet}
