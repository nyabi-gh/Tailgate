<script lang="ts">
  import EmptySearch from "./components/EmptySearch.svelte";
  import SearchField from "./components/SearchField.svelte";
  import Sidebar from "./components/Sidebar.svelte";
  import type { View } from "./lib/views";

  let view = $state<View>("search");
  let query = $state("");
  let searchField: SearchField;

  function isTyping(target: EventTarget | null) {
    return target instanceof HTMLInputElement || target instanceof HTMLTextAreaElement;
  }

  function onkeydown(event: KeyboardEvent) {
    const commandK = event.key === "k" && (event.metaKey || event.ctrlKey);
    const slash = event.key === "/" && !isTyping(event.target);
    if (commandK || slash) {
      event.preventDefault();
      view = "search";
      searchField.focus();
    }
  }

  function addTag(tag: string) {
    const tags = query.split(/\s+/).filter(Boolean);
    if (!tags.includes(tag)) query = [...tags, tag].join(" ");
    searchField.focus();
  }
</script>

<svelte:window {onkeydown} />

<div class="flex h-full">
  <Sidebar current={view} onselect={(next) => (view = next)} />

  <main class="my-2 mr-2 flex min-w-0 flex-1 flex-col overflow-hidden rounded-lg bg-panel shadow-panel">
    <header class="flex h-11 shrink-0 items-center px-3" data-tauri-drag-region>
      <SearchField bind:this={searchField} bind:value={query} />
    </header>

    <section class="min-h-0 flex-1 overflow-auto">
      {#if view === "search"}
        <EmptySearch onpick={addTag} />
      {/if}
    </section>
  </main>
</div>
