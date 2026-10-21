<script lang="ts">
  import { CircleAlert, EyeOff, Trash2 } from "@lucide/svelte";
  import { untrack } from "svelte";
  import { routeCount } from "../lib/connection.svelte";
  import { m } from "../lib/i18n/index.svelte";
  import { mediaUrl, viewerMedia } from "../lib/media";
  import type { Post } from "../lib/posts";
  import { setMuted, viewer } from "../lib/viewer.svelte";
  import { actualSize, fit, isFitted, pan, zoomAt, type View } from "../lib/zoom";

  let { post }: { post: Post } = $props();

  const media = $derived(viewerMedia(post));

  let stage: HTMLDivElement;
  let stageSize = $state({ width: 0, height: 0 });
  let view = $state<View>({ scale: 1, x: 0, y: 0 });
  /** The URL the image shows: the display one, then the original once it has decoded. */
  let shown = $state<string | null>(null);
  let original = $state<"idle" | "loading" | "failed">("idle");
  let videoFailed = $state(false);
  let dragging: { pointer: number; x: number; y: number } | null = null;

  const picture = $derived(
    media.kind === "image" || media.kind === "video" ? { width: media.width, height: media.height } : null,
  );

  // A new post starts fitted, from its display image. Only the post re-runs this, not resizes.
  $effect(() => {
    const current = media;
    const size = picture;
    untrack(() => {
      shown = current.kind === "image" ? current.display : null;
      original = "idle";
      videoFailed = false;
      if (size) view = fit(size, stageSize);
    });
  });

  $effect(() => {
    const observer = new ResizeObserver(() => {
      const fitted = picture !== null && isFitted(view, picture, stageSize);
      stageSize = { width: stage.clientWidth, height: stage.clientHeight };
      if (picture && fitted) view = fit(picture, stageSize);
    });
    observer.observe(stage);
    return () => observer.disconnect();
  });

  // A change of route cuts off whatever was loading; the new route can serve it.
  $effect(() => {
    routeCount();
    untrack(() => {
      videoFailed = false;
      if (original === "failed") original = "idle";
    });
  });

  // Zooming past what the display image holds brings in the original.
  $effect(() => {
    if (media.kind !== "image" || shown === media.original || original !== "idle") return;
    if (media.width * view.scale * devicePixelRatio > media.displayWidth) loadOriginal();
  });

  export function loadOriginal() {
    if (media.kind !== "image" || shown === media.original || original === "loading") return;
    const target = media.original;
    original = "loading";
    const image = new Image();
    image.src = mediaUrl(target);
    image
      .decode()
      .then(() => {
        if (media.kind === "image" && media.original === target) {
          shown = target;
          original = "idle";
        }
      })
      .catch(() => {
        if (media.kind === "image" && media.original === target) original = "failed";
      });
  }

  /** `null` once the original shows, or for media without one. */
  export function originalStatus() {
    return media.kind === "image" && shown !== media.original ? original : null;
  }

  export function resetZoom() {
    if (picture) view = fit(picture, stageSize);
  }

  function point(event: MouseEvent) {
    const box = stage.getBoundingClientRect();
    return { x: event.clientX - box.left, y: event.clientY - box.top };
  }

  // Not `onwheel`: zooming must cancel the scroll, which a passive listener cannot.
  $effect(() => {
    const onwheel = (event: WheelEvent) => {
      if (media.kind !== "image" || !picture) return;
      event.preventDefault();
      // Pinches arrive as Ctrl+wheel with small steps; mouse wheels step much further.
      const factor = Math.exp(-event.deltaY * (event.ctrlKey ? 0.01 : 0.002));
      view = zoomAt(view, factor, point(event), picture, stageSize);
    };
    stage.addEventListener("wheel", onwheel, { passive: false });
    return () => stage.removeEventListener("wheel", onwheel);
  });

  function onpointerdown(event: PointerEvent) {
    if (media.kind !== "image" || event.button !== 0) return;
    dragging = { pointer: event.pointerId, x: event.clientX, y: event.clientY };
    stage.setPointerCapture(event.pointerId);
  }

  function onpointermove(event: PointerEvent) {
    if (!dragging || dragging.pointer !== event.pointerId || !picture) return;
    view = pan(view, event.clientX - dragging.x, event.clientY - dragging.y, picture, stageSize);
    dragging = { pointer: event.pointerId, x: event.clientX, y: event.clientY };
  }

  function onpointerup(event: PointerEvent) {
    if (dragging?.pointer === event.pointerId) dragging = null;
  }

  function ondblclick(event: MouseEvent) {
    if (media.kind !== "image" || !picture) return;
    view = isFitted(view, picture, stageSize)
      ? actualSize(view, picture, stageSize, point(event))
      : fit(picture, stageSize);
  }
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div
  bind:this={stage}
  class={["relative size-full overflow-hidden", media.kind === "image" && "cursor-grab active:cursor-grabbing"]}
  {onpointerdown}
  {onpointermove}
  {onpointerup}
  onpointercancel={onpointerup}
  {ondblclick}
>
  {#key routeCount()}
  {#if media.kind === "image" && shown}
    <img
      src={mediaUrl(shown)}
      alt=""
      draggable="false"
      class="absolute top-0 left-0 max-w-none origin-top-left select-none"
      style:width="{media.width * view.scale}px"
      style:height="{media.height * view.scale}px"
      style:transform="translate3d({view.x}px, {view.y}px, 0)"
      style:view-transition-name="post-media"
    />
  {:else if media.kind === "video" && !videoFailed}
    <!-- svelte-ignore a11y_media_has_caption -->
    <video
      src={mediaUrl(media.url)}
      class="absolute inset-0 size-full object-contain"
      style:view-transition-name="post-media"
      autoplay
      loop
      controls
      playsinline
      muted={viewer.muted}
      onvolumechange={(event) => setMuted(event.currentTarget.muted)}
      onerror={() => (videoFailed = true)}
    ></video>
  {:else}
    <div class="grid size-full place-items-center px-8 text-dim">
      <p class="flex items-center gap-2">
        {#if media.kind === "missing" && media.deleted}
          <Trash2 size={16} strokeWidth={1.75} />{m().viewer.deleted}
        {:else if media.kind === "missing"}
          <EyeOff size={16} strokeWidth={1.75} />{m().viewer.unavailable}
        {:else if media.kind === "unsupported"}
          <CircleAlert size={16} strokeWidth={1.75} />{m().viewer.unsupported(media.ext)}
        {:else}
          <CircleAlert size={16} strokeWidth={1.75} />{m().viewer.videoFailed}
        {/if}
      </p>
    </div>
  {/if}
  {/key}
</div>
