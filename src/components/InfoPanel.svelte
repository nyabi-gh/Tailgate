<script lang="ts">
  import { ArrowDown, ArrowUp, CircleAlert, ExternalLink, Heart, LoaderCircle } from "@lucide/svelte";
  import { openUrl } from "@tauri-apps/plugin-opener";
  import { currentLocale, m } from "../lib/i18n/index.svelte";
  import type { Post, TagCategory } from "../lib/posts";
  import { categoryTone } from "../lib/tags";

  let {
    post,
    originalStatus,
    onoriginal,
  }: { post: Post; originalStatus: "idle" | "loading" | "failed" | null; onoriginal: () => void } = $props();

  const order: TagCategory[] = [
    "artist",
    "copyright",
    "character",
    "species",
    "general",
    "meta",
    "lore",
    "contributor",
    "invalid",
  ];

  const posted = $derived(
    new Intl.DateTimeFormat(currentLocale(), { dateStyle: "medium", timeStyle: "short" }).format(
      new Date(post.createdAt),
    ),
  );
  const size = $derived(
    new Intl.NumberFormat(currentLocale(), { style: "unit", unit: "megabyte", maximumFractionDigits: 1 }).format(
      post.file.size / 1_048_576,
    ),
  );
  const count = (n: number) => n.toLocaleString(currentLocale());

  function open(url: string) {
    openUrl(url).catch((cause) => console.error("could not open a source link", cause));
  }

  /** Sources are free text on e621; only web links can be opened. */
  function isLink(source: string) {
    return /^https?:\/\//i.test(source);
  }
</script>

<div class="flex flex-col gap-5 p-5 pt-14">
  <section class="flex flex-col gap-2">
    <h2 class="text-lg font-strong">#{post.id}</h2>
    <div class="flex flex-wrap items-center gap-x-4 gap-y-1 text-dim">
      <span class="flex items-center gap-1" title={m().viewer.score}>
        <ArrowUp size={14} strokeWidth={2} />{count(post.score.up)}
        <ArrowDown size={14} strokeWidth={2} class="ml-1" />{count(Math.abs(post.score.down))}
      </span>
      <span class="flex items-center gap-1" title={m().viewer.favorites}>
        <Heart size={14} strokeWidth={2} />{count(post.favCount)}
      </span>
      <span>{m().viewer.rating[post.rating]}</span>
    </div>
  </section>

  <dl class="grid grid-cols-[auto_1fr] gap-x-4 gap-y-1.5">
    <dt class="text-dim">{m().viewer.posted}</dt>
    <dd>{posted}</dd>
    {#if post.uploader.name}
      <dt class="text-dim">{m().viewer.uploader}</dt>
      <dd class="truncate">{post.uploader.name}</dd>
    {/if}
    <dt class="text-dim">{m().viewer.file}</dt>
    <dd>{post.file.width}×{post.file.height} · {size} · {post.file.ext.toUpperCase()}</dd>
  </dl>

  {#if originalStatus !== null}
    <button
      class="flex h-8 w-fit items-center gap-2 rounded-md bg-selected px-3 font-medium transition-colors hover:bg-hover disabled:opacity-60"
      disabled={originalStatus === "loading"}
      onclick={onoriginal}
    >
      {#if originalStatus === "loading"}
        <LoaderCircle size={14} strokeWidth={2} class="animate-spin" />{m().viewer.loadingOriginal}
      {:else if originalStatus === "failed"}
        <CircleAlert size={14} strokeWidth={2} />{m().viewer.originalFailed}
      {:else}
        {m().viewer.original}
      {/if}
    </button>
  {/if}

  {#if post.sources.length > 0}
    <section class="flex flex-col gap-1.5">
      <h3 class="text-xs text-faint">{m().viewer.sources}</h3>
      <ul class="flex flex-col gap-1">
        {#each post.sources as source (source)}
          <li class="min-w-0">
            {#if isLink(source)}
              <button
                class="flex w-full min-w-0 items-center gap-1 text-left text-focus hover:underline"
                title={source}
                onclick={() => open(source)}
              >
                <span class="truncate">{source.replace(/^https?:\/\/(www\.)?/i, "")}</span>
                <ExternalLink size={12} strokeWidth={1.75} class="shrink-0" />
              </button>
            {:else}
              <span class="block truncate select-text" title={source}>{source}</span>
            {/if}
          </li>
        {/each}
      </ul>
    </section>
  {/if}

  {#each order as category (category)}
    {#if post.tags[category].length > 0}
      <section class="flex flex-col gap-1.5">
        <h3 class="text-xs text-faint">{m().viewer.tags[category]}</h3>
        <ul class="flex flex-wrap gap-1">
          {#each post.tags[category] as tag (tag)}
            <li class={["rounded-sm px-1.5 py-0.5 text-sm select-text", categoryTone[category]]}>{tag}</li>
          {/each}
        </ul>
      </section>
    {/if}
  {/each}
</div>
