<script lang="ts">
  import { ZoomIn } from "@lucide/svelte";
  import { tick, untrack } from "svelte";
  import Grid from "./components/Grid.svelte";
  import Settings from "./components/Settings.svelte";
  import SignInSheet from "./components/SignInSheet.svelte";
  import Sidebar from "./components/Sidebar.svelte";
  import SortMenu from "./components/SortMenu.svelte";
  import TagInput from "./components/TagInput.svelte";
  import Toasts from "./components/Toasts.svelte";
  import Viewer from "./components/Viewer.svelte";
  import { account } from "./lib/account.svelte";
  import { onSignInNeeded } from "./lib/actions.svelte";
  import { m } from "./lib/i18n/index.svelte";
  import { CELL_WIDTH } from "./lib/layout";
  import { isMac } from "./lib/platform";
  import { favorites, startFavorites, startSearch, tagSearch, type PostList } from "./lib/search.svelte";
  import type { Post } from "./lib/posts";
  import { closeViewer, openPost, viewer } from "./lib/viewer.svelte";
  import type { View } from "./lib/views";

  let view = $state<View>("search");
  let tags = $state<string[]>([]);
  let cellWidth = $state(CELL_WIDTH.initial);
  let tagInput: TagInput | undefined = $state();
  let searchGrid: Grid | undefined = $state();
  let favoritesGrid: Grid | undefined = $state();
  /** The grid the viewer was opened from, to close back into. */
  let openedFrom: Grid | undefined;
  let signInSheet: SignInSheet;
  /** The thumbnail that carries the viewer's transition name, while it should. */
  let transitionId = $state<number | null>(null);

  function isTyping(target: EventTarget | null) {
    return target instanceof HTMLInputElement || target instanceof HTMLTextAreaElement;
  }

  /** The thumbnail grows into the viewer and shrinks back (docs/report.md §8, motion). */
  async function transition(update: () => Promise<void>) {
    if (!("startViewTransition" in document) || matchMedia("(prefers-reduced-motion: reduce)").matches) {
      await update();
      return;
    }
    await document.startViewTransition(async () => {
      await update();
      await tick();
    }).finished;
  }

  async function open(list: PostList, from: Grid | undefined, post: Post) {
    openedFrom = from;
    transitionId = post.id;
    await tick();
    await transition(async () => {
      transitionId = null;
      openPost(list, post.id);
    });
  }

  async function close() {
    const id = viewer.post?.id;
    await transition(async () => {
      closeViewer();
      if (id === undefined) return;
      await openedFrom?.reveal(id);
      transitionId = id;
    });
    transitionId = null;
  }

  function onkeydown(event: KeyboardEvent) {
    if (viewer.open) return;
    // The physical key: `key` changes with Caps Lock and the input source (Hangul, for one). Only
    // the platform's own modifier, since Ctrl K in a macOS text field deletes to the end of line.
    const commandK = event.code === "KeyK" && (isMac ? event.metaKey : event.ctrlKey);
    const slash = event.key === "/" && !isTyping(event.target);
    if (commandK || slash) {
      event.preventDefault();
      view = "search";
      // The field only exists on the search screen, so it may need a render first.
      tick().then(() => tagInput?.focus());
    }
  }

  function run(next: string[]) {
    tags = next;
    startSearch(next.join(" "));
  }

  const activeGrid = $derived(view === "favorites" ? favoritesGrid : searchGrid);
  const showsGrid = $derived(
    (view === "search" && tagSearch.query !== null) || (view === "favorites" && favorites.query !== null),
  );

  // Favorites and votes in the results belong to whoever is signed in, so a change of account
  // reloads the search and the favorites.
  const signedInAs = $derived.by(() => {
    const current = account();
    return current.state === "signedIn" ? current.name : null;
  });
  let previousAccount: string | null | undefined;
  $effect(() => {
    const now = signedInAs;
    untrack(() => {
      if (previousAccount !== undefined && previousAccount !== now) {
        if (tagSearch.query !== null) startSearch(tagSearch.query);
        if (now !== null && favorites.query !== null) startFavorites();
      }
      previousAccount = now;
    });
  });

  onSignInNeeded(() => signInSheet.open());

  // A search without tags is e621's front page, the newest posts; the app opens on it.
  startSearch("");

  // The favorites load the first time their screen opens while signed in.
  $effect(() => {
    if (view === "favorites" && signedInAs !== null && untrack(() => favorites.query) === null) startFavorites();
  });
</script>

<svelte:window {onkeydown} />

<div class="flex h-full">
  <Sidebar current={view} onselect={(next) => (view = next)} onaccount={() => signInSheet.open()} />

  <main class="my-2 mr-2 flex min-w-0 flex-1 flex-col overflow-hidden rounded-lg bg-panel shadow-panel">
    <header class="flex min-h-11 shrink-0 items-center gap-2 px-3 py-1" data-tauri-drag-region>
      {#if view === "favorites" || view === "settings"}
        <h1 class="px-1 text-base font-strong">{m().nav[view]}</h1>
      {:else}
        <TagInput bind:this={tagInput} bind:tags onsubmit={run} />
        <SortMenu {tags} onchange={run} />
      {/if}
      {#if showsGrid}
        <label class="ml-auto flex shrink-0 items-center gap-2 text-dim" title={m().search.size}>
          <ZoomIn size={15} strokeWidth={1.75} />
          <span class="sr-only">{m().search.size}</span>
          <input
            type="range"
            class="w-24 accent-focus"
            min={CELL_WIDTH.min}
            max={CELL_WIDTH.max}
            step="10"
            value={cellWidth}
            oninput={(event) => activeGrid?.resize(Number(event.currentTarget.value))}
          />
        </label>
      {/if}
    </header>

    <!-- Both grids stay mounted, so each screen keeps its scroll position. -->
    <section class="relative min-h-0 flex-1">
      <div class={["absolute inset-0", view !== "search" && "invisible"]}>
        {#if tagSearch.query !== null}
          <Grid
            bind:this={searchGrid}
            list={tagSearch}
            bind:cellWidth
            {transitionId}
            empty={m().search.noResults}
            onopen={(post) => open(tagSearch, searchGrid, post)}
          />
        {/if}
      </div>
      <div class={["absolute inset-0", view !== "favorites" && "invisible"]}>
        {#if signedInAs === null}
          <div class="grid h-full place-items-center px-8 pb-[12vh] text-center">
            <div class="flex flex-col items-center gap-3">
              <p class="text-dim">{m().favorites.signedOut}</p>
              <button
                class="h-8 rounded-md bg-focus px-3.5 font-medium text-panel"
                onclick={() => signInSheet.open()}>{m().account.signIn}</button
              >
            </div>
          </div>
        {:else if favorites.query !== null}
          <Grid
            bind:this={favoritesGrid}
            list={favorites}
            bind:cellWidth
            {transitionId}
            empty={m().favorites.empty}
            onopen={(post) => open(favorites, favoritesGrid, post)}
          />
        {/if}
      </div>
      {#if view === "settings"}
        <div class="absolute inset-0 overflow-y-auto"><Settings onsignin={() => signInSheet.open()} /></div>
      {/if}
    </section>
  </main>
</div>

{#if viewer.open}
  <Viewer onclose={close} />
{/if}

<SignInSheet bind:this={signInSheet} />
<Toasts />
