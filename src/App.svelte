<script lang="ts">
  import { ZoomIn } from "@lucide/svelte";
  import { untrack } from "svelte";
  import EmptySearch from "./components/EmptySearch.svelte";
  import Grid from "./components/Grid.svelte";
  import SignInSheet from "./components/SignInSheet.svelte";
  import Sidebar from "./components/Sidebar.svelte";
  import SortMenu from "./components/SortMenu.svelte";
  import TagInput from "./components/TagInput.svelte";
  import { account } from "./lib/account.svelte";
  import { m } from "./lib/i18n/index.svelte";
  import { CELL_WIDTH } from "./lib/layout";
  import { isMac } from "./lib/platform";
  import { search, startSearch } from "./lib/search.svelte";
  import type { View } from "./lib/views";

  let view = $state<View>("search");
  let tags = $state<string[]>([]);
  let cellWidth = $state(CELL_WIDTH.initial);
  let tagInput: TagInput;
  let grid: Grid | undefined = $state();
  let signInSheet: SignInSheet;

  function isTyping(target: EventTarget | null) {
    return target instanceof HTMLInputElement || target instanceof HTMLTextAreaElement;
  }

  function onkeydown(event: KeyboardEvent) {
    // The physical key: `key` changes with Caps Lock and the input source (Hangul, for one). Only
    // the platform's own modifier, since Ctrl K in a macOS text field deletes to the end of line.
    const commandK = event.code === "KeyK" && (isMac ? event.metaKey : event.ctrlKey);
    const slash = event.key === "/" && !isTyping(event.target);
    if (commandK || slash) {
      event.preventDefault();
      view = "search";
      tagInput.focus();
    }
  }

  function addTag(tag: string) {
    if (!tags.includes(tag)) tags = [...tags, tag];
    tagInput.focus();
  }

  function run(next: string[]) {
    tags = next;
    startSearch(next.join(" "));
  }

  // Favorites and votes in the results belong to whoever is signed in, so a change of account
  // reloads the search.
  const signedInAs = $derived.by(() => {
    const current = account();
    return current.state === "signedIn" ? current.name : null;
  });
  let previousAccount: string | null | undefined;
  $effect(() => {
    const now = signedInAs;
    untrack(() => {
      if (previousAccount !== undefined && previousAccount !== now && search.query !== null) {
        startSearch(search.query);
      }
      previousAccount = now;
    });
  });
</script>

<svelte:window {onkeydown} />

<div class="flex h-full">
  <Sidebar current={view} onselect={(next) => (view = next)} onaccount={() => signInSheet.open()} />

  <main class="my-2 mr-2 flex min-w-0 flex-1 flex-col overflow-hidden rounded-lg bg-panel shadow-panel">
    <header class="flex min-h-11 shrink-0 items-center gap-2 px-3 py-1" data-tauri-drag-region>
      <TagInput bind:this={tagInput} bind:tags onsubmit={run} />
      <SortMenu {tags} onchange={run} />
      {#if search.query !== null}
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
            oninput={(event) => grid?.resize(Number(event.currentTarget.value))}
          />
        </label>
      {/if}
    </header>

    <section class="min-h-0 flex-1">
      {#if view === "search"}
        {#if search.query === null}
          <div class="h-full overflow-auto"><EmptySearch onpick={addTag} /></div>
        {:else}
          <Grid bind:this={grid} bind:cellWidth />
        {/if}
      {/if}
    </section>
  </main>
</div>

<SignInSheet bind:this={signInSheet} />
