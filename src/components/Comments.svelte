<script lang="ts">
  import { LoaderCircle, Pin } from "@lucide/svelte";
  import { CommentList } from "../lib/comments.svelte";
  import { apiErrorMessage } from "../lib/errors";
  import { currentLocale, m } from "../lib/i18n/index.svelte";
  import type { Post } from "../lib/posts";
  import DText from "./DText.svelte";

  let { post, onsearch }: { post: Post; onsearch: (tags: string[]) => void } = $props();

  /** How long a post has to stay open before its comments are asked for. */
  const SETTLE_MS = 300;

  let list = $state.raw<CommentList | null>(null);

  // A favorite or a vote replaces the post object; only another post is a reason to ask again.
  const id = $derived(post.id);
  const hasComments = $derived(post.commentCount > 0);

  $effect(() => {
    const of = id;
    list = null;
    if (!hasComments) return;
    // Stepping with a held key passes many posts, and each one's comments would be a request
    // in the API's one-a-second queue, ahead of the pages the grid and the viewer wait for.
    const timer = setTimeout(() => {
      const next = new CommentList(of);
      list = next;
      next.loadMore();
    }, SETTLE_MS);
    return () => clearTimeout(timer);
  });

  const ordered = $derived.by(() => {
    const items = list?.items ?? [];
    return [...items.filter((comment) => comment.isSticky), ...items.filter((comment) => !comment.isSticky)];
  });
  const empty = $derived(!hasComments || (list !== null && list.done && list.items.length === 0));

  const dates = $derived(new Intl.DateTimeFormat(currentLocale(), { dateStyle: "medium" }));
</script>

<div class="flex flex-col gap-4 p-5 pt-2">
  {#if empty}
    <p class="text-dim">{m().comments.none}</p>
  {:else}
    {#if ordered.length > 0}
      <ul class="flex flex-col gap-4">
        {#each ordered as comment (comment.id)}
          <li class="flex flex-col gap-1">
            <div class="flex items-baseline gap-2 text-sm">
              {#if comment.isSticky}
                <span class="shrink-0 self-center text-focus" title={m().comments.sticky}>
                  <Pin size={12} strokeWidth={2} aria-label={m().comments.sticky} />
                </span>
              {/if}
              <span class="min-w-0 truncate font-medium">{comment.creator ?? m().comments.unknownUser}</span>
              <span class="shrink-0 text-faint">{dates.format(new Date(comment.createdAt))}</span>
              {#if comment.score !== 0}
                <span class="ml-auto shrink-0 text-dim tabular-nums">
                  {comment.score > 0 ? "+" : "−"}{Math.abs(comment.score).toLocaleString(currentLocale())}
                </span>
              {/if}
            </div>
            <DText source={comment.body} {onsearch} />
          </li>
        {/each}
      </ul>
    {/if}

    {#if list === null || list.loading}
      <p class="flex items-center gap-2 text-dim">
        <LoaderCircle size={14} strokeWidth={2} class="animate-spin" />{m().comments.loading}
      </p>
    {:else if list.error !== null}
      <div class="flex flex-col items-start gap-2">
        <p class="text-dim">{apiErrorMessage(list.error)}</p>
        <button
          class="h-8 rounded-md bg-selected px-3 font-medium transition-colors hover:bg-hover"
          onclick={() => list?.loadMore()}
        >
          {m().search.retry}
        </button>
      </div>
    {:else if !list.done}
      <button
        class="h-8 w-fit rounded-md bg-selected px-3 font-medium transition-colors hover:bg-hover"
        onclick={() => list?.loadMore()}
      >
        {m().comments.more}
      </button>
    {/if}
  {/if}
</div>
