<script lang="ts">
  import { EyeOff, Heart, ImageOff, Play, Trash2 } from "@lucide/svelte";
  import { m } from "../lib/i18n/index.svelte";
  import { mediaUrl } from "../lib/media";
  import type { Post } from "../lib/posts";

  let {
    post,
    x,
    y,
    width,
    height,
    large,
  }: { post: Post; x: number; y: number; width: number; height: number; large: boolean } = $props();

  // The 256px-wide preview looks soft in large cells on a 2x screen; the 850px sample is several
  // times the download, so it is used only then (docs/report.md §5).
  const source = $derived(large ? (post.sample?.url ?? post.preview.url) : post.preview.url);
  // Per source, so a failed preview does not hide the sample when the rows grow.
  let failedSource = $state<string | null>(null);
  const failed = $derived(source !== null && failedSource === source);

  function duration(seconds: number): string {
    const whole = Math.round(seconds);
    return `${Math.floor(whole / 60)}:${String(whole % 60).padStart(2, "0")}`;
  }

  const badge = "flex h-5 items-center gap-1 rounded-sm bg-window/80 px-1.5 text-xs font-medium text-ink";
</script>

<div
  class="absolute overflow-hidden bg-raised"
  style:left="{x}px"
  style:top="{y}px"
  style:width="{width}px"
  style:height="{height}px"
>
  {#if source && !failed}
    <img
      src={mediaUrl(source)}
      alt=""
      decoding="async"
      draggable="false"
      class="size-full object-cover"
      onerror={() => (failedSource = source)}
    />
  {:else}
    <div class="grid size-full place-items-center text-faint" title={post.deleted ? m().search.deleted : m().search.unavailable}>
      {#if post.deleted}
        <Trash2 size={18} strokeWidth={1.5} />
      {:else if failed}
        <ImageOff size={18} strokeWidth={1.5} />
      {:else}
        <EyeOff size={18} strokeWidth={1.5} />
      {/if}
    </div>
  {/if}

  <div class="absolute top-1.5 right-1.5 flex gap-1">
    {#if post.isFavorited}
      <span class={badge}><Heart size={11} strokeWidth={2.25} class="fill-current" /></span>
    {/if}
    {#if post.video !== null && post.duration !== null}
      <span class={badge}><Play size={10} strokeWidth={2.25} class="fill-current" />{duration(post.duration)}</span>
    {:else if post.file.ext === "gif"}
      <span class={badge}>GIF</span>
    {/if}
  </div>
</div>
