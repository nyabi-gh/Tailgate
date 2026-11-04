<script lang="ts">
  import { ChevronLeft, ChevronRight, PanelRightClose, PanelRightOpen, X } from "@lucide/svelte";
  import { m } from "../lib/i18n/index.svelte";
  import { mediaUrl } from "../lib/media";
  import { download, signedIn, toggleFavorite, vote } from "../lib/actions.svelte";
  import { setMenu } from "../lib/sets.svelte";
  import { isEditable, onKeys } from "../lib/shortcuts";
  import { step, togglePanel, viewer } from "../lib/viewer.svelte";
  import InfoPanel from "./InfoPanel.svelte";
  import MediaStage from "./MediaStage.svelte";

  let {
    onclose,
    onpool,
    ontag,
  }: { onclose: () => void; onpool: (id: number) => void; ontag: (tag: string) => void } = $props();

  let stage: MediaStage | undefined = $state();

  // The menu belongs to this viewing; the next one starts without it.
  $effect(() => () => (setMenu.open = false));

  function toggleSetMenu() {
    if (!signedIn()) return;
    setMenu.open = !setMenu.open;
    if (setMenu.open && !viewer.panelOpen) togglePanel();
  }

  // Neighbours' display images are fetched ahead, so stepping shows them at once.
  $effect(() => {
    for (const neighbour of [viewer.posts[viewer.index - 1], viewer.posts[viewer.index + 1]]) {
      const url = neighbour?.file.ext === "gif" ? null : neighbour?.sample?.url;
      if (url) new Image().src = mediaUrl(url);
    }
  });

  function onkeydown(event: KeyboardEvent) {
    if (event.isComposing || event.metaKey || event.ctrlKey || event.altKey || isEditable(event.target)) return false;
    const action = keyAction(event);
    if (action === undefined) return false;
    event.preventDefault();
    // A held key repeats; only stepping should, the rest would flip back and forth (and each
    // favorite or vote is a request to e621).
    if (!event.repeat || event.key === "ArrowLeft" || event.key === "ArrowRight") action();
    return true;
  }

  function keyAction(event: KeyboardEvent): (() => void) | undefined {
    // The video's own controls use the arrow keys to seek and to change the volume.
    const inVideo = event.target instanceof HTMLVideoElement;
    const post = viewer.post;
    if (event.key === "Escape" && setMenu.open) return () => (setMenu.open = false);
    if (event.key === "Escape") return onclose;
    if (event.key === "ArrowUp" && !inVideo && post) return () => vote(post, true);
    if (event.key === "ArrowDown" && !inVideo && post) return () => vote(post, false);
    if (event.code === "KeyF" && post) return () => toggleFavorite(post);
    if (event.code === "KeyD" && post) return () => download(post);
    if (event.code === "KeyS") return toggleSetMenu;
    if (event.key === "ArrowLeft" && !inVideo) return () => step(-1);
    if (event.key === "ArrowRight" && !inVideo) return () => step(1);
    if (event.code === "KeyI") return togglePanel;
    if (event.code === "Digit0") return () => stage?.resetZoom();
    return undefined;
  }

  $effect(() => onKeys("viewer", onkeydown));

  const control =
    "grid size-9 place-items-center rounded-full bg-panel/80 text-ink shadow-panel transition-opacity hover:opacity-100";
</script>

{#if viewer.post}
  {@const post = viewer.post}
  <div class="fixed inset-0 z-30 flex bg-window" role="dialog" aria-modal="true" aria-label="#{post.id}">
    <div class="group relative min-w-0 flex-1">
      {#key post.id}
        <MediaStage bind:this={stage} {post} />
      {/key}

      <div class="absolute inset-x-0 top-0 h-12" data-tauri-drag-region></div>

      {#if viewer.total !== null}
        <span
          class="pointer-events-none absolute top-4 left-1/2 -translate-x-1/2 rounded-full bg-panel/80 px-3 py-1 text-sm font-medium tabular-nums shadow-panel"
        >
          {m().viewer.position(viewer.index + 1, viewer.total)}
        </span>
      {/if}

      <div class="absolute top-3 right-3 flex gap-2">
        <button
          class={[control, "opacity-60"]}
          aria-label={viewer.panelOpen ? m().viewer.hidePanel : m().viewer.showPanel}
          title={viewer.panelOpen ? m().viewer.hidePanel : m().viewer.showPanel}
          onclick={togglePanel}
        >
          {#if viewer.panelOpen}<PanelRightClose size={17} strokeWidth={1.75} />{:else}<PanelRightOpen
              size={17}
              strokeWidth={1.75}
            />{/if}
        </button>
        <button class={[control, "opacity-60"]} aria-label={m().viewer.close} title={m().viewer.close} onclick={onclose}>
          <X size={17} strokeWidth={1.75} />
        </button>
      </div>

      {#if viewer.hasPrevious}
        <button
          class={[control, "absolute top-1/2 left-3 -translate-y-1/2 opacity-0 group-hover:opacity-60"]}
          aria-label={m().viewer.previous}
          onclick={() => step(-1)}
        >
          <ChevronLeft size={20} strokeWidth={1.75} />
        </button>
      {/if}
      {#if viewer.hasNext}
        <button
          class={[control, "absolute top-1/2 right-3 -translate-y-1/2 opacity-0 group-hover:opacity-60"]}
          aria-label={m().viewer.next}
          onclick={() => step(1)}
        >
          <ChevronRight size={20} strokeWidth={1.75} />
        </button>
      {/if}
    </div>

    {#if viewer.panelOpen}
      <aside class="w-[300px] shrink-0 overflow-y-auto border-l border-seam bg-panel">
        <InfoPanel
          {post}
          originalStatus={stage?.originalStatus() ?? null}
          onoriginal={() => stage?.loadOriginal()}
          {onpool}
          {ontag}
        />
      </aside>
    {/if}
  </div>
{/if}
