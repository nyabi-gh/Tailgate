<script lang="ts">
  import { BookOpen, ChevronLeft, ExternalLink, ZoomIn } from "@lucide/svelte";
  import { openUrl } from "@tauri-apps/plugin-opener";
  import { tick, untrack } from "svelte";
  import Grid from "./components/Grid.svelte";
  import PoolList from "./components/PoolList.svelte";
  import PoolsBar from "./components/PoolsBar.svelte";
  import PopularBar from "./components/PopularBar.svelte";
  import SetList from "./components/SetList.svelte";
  import SetsBar from "./components/SetsBar.svelte";
  import Settings from "./components/Settings.svelte";
  import SignInSheet from "./components/SignInSheet.svelte";
  import Sidebar from "./components/Sidebar.svelte";
  import SortMenu from "./components/SortMenu.svelte";
  import TagInput from "./components/TagInput.svelte";
  import Toasts from "./components/Toasts.svelte";
  import UpdateSheet from "./components/UpdateSheet.svelte";
  import Viewer from "./components/Viewer.svelte";
  import { account } from "./lib/account.svelte";
  import { onSignInNeeded } from "./lib/actions.svelte";
  import { canLoadRemote } from "./lib/connection.svelte";
  import { m } from "./lib/i18n/index.svelte";
  import { CELL_WIDTH } from "./lib/layout";
  import { isMac } from "./lib/platform";
  import {
    favorites,
    poolPosts,
    popular,
    setPosts,
    startFavorites,
    startPopular,
    startSearch,
    tagSearch,
    type PostList,
  } from "./lib/search.svelte";
  import { cellWidth, rememberSearch, setCellWidth } from "./lib/settings.svelte";
  import type { Post } from "./lib/posts";
  import { closePool, openedPool, openPool, poolIndex } from "./lib/pools.svelte";
  import {
    closeSet,
    markMySetsStale,
    mySets,
    mySetsStale,
    openedSet,
    openSet,
    reloadMySets,
    setIndex,
    setMode,
  } from "./lib/sets.svelte";
  import { checkOnceAutomatically } from "./lib/updates.svelte";
  import { isEditable, onKeys } from "./lib/shortcuts";
  import { closeViewer, isViewing, openPost, viewer } from "./lib/viewer.svelte";
  import type { View } from "./lib/views";

  let view = $state<View>("search");
  let tags = $state<string[]>([]);
  let tagInput: TagInput | undefined = $state();
  let searchGrid: Grid | undefined = $state();
  let favoritesGrid: Grid | undefined = $state();
  let popularGrid: Grid | undefined = $state();
  let poolGrid: Grid | undefined = $state();
  let setGrid: Grid | undefined = $state();
  /** The grid the viewer was opened from, to close back into. */
  let openedFrom: Grid | undefined;
  let signInSheet: SignInSheet;
  /** The thumbnail that carries the viewer's transition name, while it should. */
  let transitionId = $state<number | null>(null);

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
    for (const start of deferredStarts.values()) start();
    deferredStarts.clear();
  }

  /**
   * Starts that would empty the list under the viewer wait until it closes (and has shrunk back
   * into the thumbnail), so the post on screen stays. The latest per list wins.
   */
  const deferredStarts = new Map<PostList, () => void>();

  function restart(list: PostList, start: () => void) {
    if (isViewing(list)) deferredStarts.set(list, start);
    else start();
  }

  function onkeydown(event: KeyboardEvent) {
    // The physical key: `key` changes with Caps Lock and the input source (Hangul, for one). Only
    // the platform's own modifier, since Ctrl K in a macOS text field deletes to the end of line.
    const commandK = event.code === "KeyK" && (isMac ? event.metaKey : event.ctrlKey);
    const slash = event.key === "/" && !isEditable(event.target);
    if (!commandK && !slash) return false;
    event.preventDefault();
    view = "search";
    // The field only exists on the search screen, so it may need a render first.
    tick().then(() => tagInput?.focus());
    return true;
  }

  $effect(() => onKeys("app", onkeydown));

  function run(next: string[]) {
    tags = next;
    const query = next.join(" ");
    rememberSearch(query);
    startSearch(query);
  }

  const activeGrid = $derived(
    view === "favorites"
      ? favoritesGrid
      : view === "popular"
        ? popularGrid
        : view === "pools"
          ? poolGrid
          : view === "sets"
            ? setGrid
            : searchGrid,
  );
  const pool = $derived(openedPool());
  const set = $derived(openedSet());
  const showsGrid = $derived(
    (view === "search" && tagSearch.query !== null) ||
      (view === "favorites" && favorites.query !== null) ||
      (view === "popular" && popular.query !== null) ||
      (view === "pools" && pool !== null) ||
      (view === "sets" && set !== null),
  );

  /** From a post's pool in the viewer: the pool screen opens on it. */
  async function showPool(id: number) {
    await close();
    view = "pools";
    openPool(id);
  }

  /** From a tag in the viewer: a new search for that tag alone. */
  async function searchTag(tag: string) {
    await close();
    view = "search";
    run([tag]);
  }

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
        const query = tagSearch.query;
        if (query !== null) restart(tagSearch, () => startSearch(query));
        if (now !== null && favorites.query !== null) restart(favorites, startFavorites);
        markMySetsStale();
      }
      previousAccount = now;
    });
  });

  onSignInNeeded(() => signInSheet.open());

  // A search without tags is e621's front page, the newest posts; the app opens on it.
  startSearch("");

  // The update check goes through the proxy too, so it waits for a route.
  $effect(() => {
    if (canLoadRemote()) untrack(checkOnceAutomatically);
  });

  // The popular posts load the first time their screen opens.
  $effect(() => {
    if (view === "popular" && untrack(() => popular.query) === null) startPopular();
  });

  // The pool list loads the first time its screen opens.
  $effect(() => {
    if (view === "pools" && !untrack(() => poolIndex.started)) poolIndex.search({});
  });

  // Each list of the set screen loads the first time it shows; the user's own again after a change.
  $effect(() => {
    if (view !== "sets" || openedSet() !== null) return;
    if (setMode.mode === "search" && !untrack(() => setIndex.started)) setIndex.search();
    if (setMode.mode === "mine" && signedInAs !== null && (mySetsStale() || !untrack(() => mySets.started))) {
      untrack(reloadMySets);
    }
  });

  // The favorites load the first time their screen opens while signed in.
  $effect(() => {
    if (view === "favorites" && signedInAs !== null && untrack(() => favorites.query) === null) startFavorites();
  });
</script>

<div class="flex h-full">
  <Sidebar current={view} onselect={(next) => (view = next)} onaccount={() => signInSheet.open()} />

  <main class="my-2 mr-2 flex min-w-0 flex-1 flex-col overflow-hidden rounded-lg bg-panel shadow-panel">
    <header class="flex min-h-11 shrink-0 items-center gap-2 px-3 pt-2.5 pb-1" data-tauri-drag-region>
      {#if view === "favorites" || view === "settings"}
        <h1 class="px-1 text-base font-strong">{m().nav[view]}</h1>
      {:else if view === "popular"}
        <PopularBar />
      {:else if view === "pools" && pool !== null}
        <button
          class="flex h-8 shrink-0 items-center gap-0.5 rounded-md pr-2.5 pl-1.5 text-ink/80 transition-colors hover:bg-hover"
          onclick={closePool}
        >
          <ChevronLeft size={16} strokeWidth={2} />{m().pools.back}
        </button>
        <div class="flex min-w-0 items-baseline gap-2">
          <h1 class="truncate text-base font-strong">{pool.pool?.name ?? "…"}</h1>
          {#if pool.pool && pool.postCount !== null}
            <span class="shrink-0 text-dim">
              {m().pools.category[pool.pool.category]} · {m().pools.posts(pool.postCount)}
            </span>
          {/if}
        </div>
        <button
          class="ml-3 flex h-8 shrink-0 items-center gap-1.5 rounded-md px-2.5 font-medium text-ink/80 transition-colors hover:bg-hover disabled:opacity-50"
          disabled={poolPosts.posts.length === 0}
          onclick={() => {
            const first = poolPosts.posts[0];
            if (first) open(poolPosts, poolGrid, first);
          }}
        >
          <BookOpen size={15} strokeWidth={1.75} />{m().pools.read}
        </button>
        <button
          class="flex h-8 shrink-0 items-center gap-1.5 rounded-md px-2.5 text-ink/80 transition-colors hover:bg-hover"
          onclick={() =>
            openUrl(`https://e621.net/pools/${pool.id}`).catch((cause) => console.error("could not open the pool page", cause))}
        >
          <ExternalLink size={15} strokeWidth={1.75} />{m().pools.onE621}
        </button>
      {:else if view === "pools"}
        <PoolsBar />
      {:else if view === "sets" && set !== null}
        <button
          class="flex h-8 shrink-0 items-center gap-0.5 rounded-md pr-2.5 pl-1.5 text-ink/80 transition-colors hover:bg-hover"
          onclick={closeSet}
        >
          <ChevronLeft size={16} strokeWidth={2} />{m().sets.back}
        </button>
        <div class="flex min-w-0 items-baseline gap-2">
          <h1 class="truncate text-base font-strong">{set.set?.name ?? "…"}</h1>
          {#if set.set && set.postCount !== null}
            <span class="shrink-0 text-dim">set:{set.set.shortname} · {m().sets.posts(set.postCount)}</span>
          {/if}
        </div>
        <button
          class="ml-3 flex h-8 shrink-0 items-center gap-1.5 rounded-md px-2.5 text-ink/80 transition-colors hover:bg-hover"
          onclick={() =>
            openUrl(`https://e621.net/post_sets/${set.id}`).catch((cause) => console.error("could not open the set page", cause))}
        >
          <ExternalLink size={15} strokeWidth={1.75} />{m().sets.onE621}
        </button>
      {:else if view === "sets"}
        <SetsBar />
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
            value={cellWidth()}
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
            bind:cellWidth={cellWidth, setCellWidth}
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
            bind:cellWidth={cellWidth, setCellWidth}
            {transitionId}
            empty={m().favorites.empty}
            onopen={(post) => open(favorites, favoritesGrid, post)}
          />
        {/if}
      </div>
      <div class={["absolute inset-0", view !== "popular" && "invisible"]}>
        {#if popular.query !== null}
          <Grid
            bind:this={popularGrid}
            list={popular}
            bind:cellWidth={cellWidth, setCellWidth}
            {transitionId}
            empty={m().popular.empty}
            onopen={(post) => open(popular, popularGrid, post)}
          />
        {/if}
      </div>
      <div class={["absolute inset-0", (view !== "pools" || pool !== null) && "invisible"]}>
        <PoolList onopen={openPool} />
      </div>
      <div class={["absolute inset-0", (view !== "pools" || pool === null) && "invisible"]}>
        {#if pool !== null && poolPosts.query !== null}
          <Grid
            bind:this={poolGrid}
            list={poolPosts}
            bind:cellWidth={cellWidth, setCellWidth}
            {transitionId}
            empty={m().pools.emptyPool}
            onopen={(post) => open(poolPosts, poolGrid, post)}
          />
        {/if}
      </div>
      <div class={["absolute inset-0", (view !== "sets" || set !== null || setMode.mode !== "mine") && "invisible"]}>
        {#if signedInAs === null}
          <div class="grid h-full place-items-center px-8 pb-[12vh] text-center">
            <div class="flex flex-col items-center gap-3">
              <p class="text-dim">{m().sets.signedOut}</p>
              <button
                class="h-8 rounded-md bg-focus px-3.5 font-medium text-panel"
                onclick={() => signInSheet.open()}>{m().account.signIn}</button
              >
            </div>
          </div>
        {:else}
          <SetList list={mySets} mine empty={m().sets.emptyMine} onopen={openSet} />
        {/if}
      </div>
      <div class={["absolute inset-0", (view !== "sets" || set !== null || setMode.mode !== "search") && "invisible"]}>
        <SetList list={setIndex} mine={false} empty={m().sets.empty} onopen={openSet} />
      </div>
      <div class={["absolute inset-0", (view !== "sets" || set === null) && "invisible"]}>
        {#if set !== null && setPosts.query !== null}
          <Grid
            bind:this={setGrid}
            list={setPosts}
            bind:cellWidth={cellWidth, setCellWidth}
            {transitionId}
            empty={m().sets.emptySet}
            onopen={(post) => open(setPosts, setGrid, post)}
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
  <Viewer onclose={close} onpool={showPool} ontag={searchTag} />
{/if}

<SignInSheet bind:this={signInSheet} />
<UpdateSheet />
<Toasts />
