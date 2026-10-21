<script lang="ts">
  import { ChevronLeft, ChevronRight, PanelRightClose, PanelRightOpen, X } from "@lucide/svelte";
  import { m } from "../lib/i18n/index.svelte";
  import { mediaUrl } from "../lib/media";
  import { search } from "../lib/search.svelte";
  import { step, togglePanel, viewer } from "../lib/viewer.svelte";
  import InfoPanel from "./InfoPanel.svelte";
  import MediaStage from "./MediaStage.svelte";

  let { onclose }: { onclose: () => void } = $props();

  let stage: MediaStage | undefined = $state();

  // Neighbours' display images are fetched ahead, so stepping shows them at once.
  $effect(() => {
    for (const neighbour of [search.posts[viewer.index - 1], search.posts[viewer.index + 1]]) {
      const url = neighbour?.file.ext === "gif" ? null : neighbour?.sample?.url;
      if (url) new Image().src = mediaUrl(url);
    }
  });

  function onkeydown(event: KeyboardEvent) {
    if (event.isComposing || event.metaKey || event.ctrlKey || event.altKey) return;
    // The video's own controls use the arrow keys to seek.
    const inVideo = event.target instanceof HTMLVideoElement;
    if (event.key === "Escape") onclose();
    else if (event.key === "ArrowLeft" && !inVideo) step(-1);
    else if (event.key === "ArrowRight" && !inVideo) step(1);
    else if (event.code === "KeyI") togglePanel();
    else if (event.code === "Digit0") stage?.resetZoom();
    else return;
    event.preventDefault();
  }

  const control =
    "grid size-9 place-items-center rounded-full bg-panel/80 text-ink shadow-panel transition-opacity hover:opacity-100";
</script>

<svelte:window {onkeydown} />

{#if viewer.post}
  {@const post = viewer.post}
  <div class="fixed inset-0 z-30 flex bg-window" role="dialog" aria-modal="true" aria-label="#{post.id}">
    <div class="group relative min-w-0 flex-1">
      {#key post.id}
        <MediaStage bind:this={stage} {post} />
      {/key}

      <div class="absolute inset-x-0 top-0 h-12" data-tauri-drag-region></div>

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
        />
      </aside>
    {/if}
  </div>
{/if}
