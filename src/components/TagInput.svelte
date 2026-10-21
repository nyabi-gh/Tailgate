<script lang="ts">
  import { Search, X } from "@lucide/svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { currentLocale, m } from "../lib/i18n/index.svelte";
  import { shortcutLabel } from "../lib/platform";
  import type { TagSuggestion } from "../lib/posts";
  import { knownCategory, rememberCategories } from "../lib/search.svelte";
  import { categoryDot, categoryTone, parseTag } from "../lib/tags";

  let { tags = $bindable([]), onsubmit }: { tags?: string[]; onsubmit: (tags: string[]) => void } = $props();

  /** e621 answers suggestions from three characters on. */
  const MIN_SUGGEST_LENGTH = 3;
  const SUGGEST_DELAY_MS = 300;

  let input: HTMLInputElement;
  let draft = $state("");
  let suggestions = $state.raw<TagSuggestion[]>([]);
  let highlighted = $state(0);
  let open = $state(false);
  const cache = new Map<string, TagSuggestion[]>();
  let timer: ReturnType<typeof setTimeout> | undefined;

  export function focus() {
    input.focus();
  }

  function tone(text: string): string {
    const token = parseTag(text);
    if (token.metatag) return categoryTone.meta;
    return categoryTone[knownCategory(token.name) ?? "unknown"];
  }

  function commit(text: string) {
    const tag = text.trim();
    if (tag && !tags.includes(tag)) tags = [...tags, tag];
    draft = "";
    closeSuggestions();
  }

  function remove(index: number) {
    tags = tags.filter((_, i) => i !== index);
    input.focus();
  }

  function closeSuggestions() {
    open = false;
    suggestions = [];
    clearTimeout(timer);
  }

  function accept(suggestion: TagSuggestion) {
    const prefix = draft.trim().match(/^[-~]/)?.[0] ?? "";
    commit(prefix + suggestion.name);
  }

  function submit() {
    if (draft.trim()) commit(draft);
    closeSuggestions();
    onsubmit(tags);
  }

  /** Typing or pasting a space ends a tag. */
  function oninput() {
    const parts = draft.split(/\s+/);
    if (parts.length > 1) {
      const last = parts.pop() ?? "";
      for (const part of parts) commit(part);
      draft = last;
    }
    suggest();
  }

  function suggest() {
    clearTimeout(timer);
    const token = parseTag(draft.trim());
    if (token.metatag || token.name.length < MIN_SUGGEST_LENGTH) {
      closeSuggestions();
      return;
    }
    const text = token.name.toLowerCase();
    const cached = cache.get(text);
    if (cached) {
      show(cached);
      return;
    }
    timer = setTimeout(async () => {
      const found = await invoke<TagSuggestion[]>("autocomplete", { text }).catch((cause) => {
        console.error("tag suggestions failed", cause);
        return null;
      });
      if (found === null) return;
      cache.set(text, found);
      rememberCategories(Object.fromEntries(found.map((tag) => [tag.name, tag.category])));
      // Only if the field still asks for these.
      if (parseTag(draft.trim()).name.toLowerCase() === text) show(found);
    }, SUGGEST_DELAY_MS);
  }

  function show(found: TagSuggestion[]) {
    suggestions = found.slice(0, 8);
    highlighted = 0;
    open = suggestions.length > 0;
  }

  function onkeydown(event: KeyboardEvent) {
    // Keys pressed while a Hangul or other IME composition is open belong to the composition.
    if (event.isComposing) return;
    if (open && (event.key === "ArrowDown" || event.key === "ArrowUp")) {
      event.preventDefault();
      const step = event.key === "ArrowDown" ? 1 : -1;
      highlighted = (highlighted + step + suggestions.length) % suggestions.length;
    } else if (open && (event.key === "Tab" || event.key === "Enter")) {
      event.preventDefault();
      const choice = suggestions[highlighted];
      if (choice !== undefined) accept(choice);
    } else if (event.key === "Enter") {
      event.preventDefault();
      submit();
    } else if (event.key === "Escape") {
      if (open) closeSuggestions();
      else input.blur();
    } else if (event.key === "Backspace" && draft === "" && tags.length > 0) {
      event.preventDefault();
      tags = tags.slice(0, -1);
    }
  }
</script>

<div class="relative w-full max-w-[640px]">
  <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
  <div
    class="flex min-h-9 w-full flex-wrap items-center gap-1 rounded-md bg-raised py-1 pr-2 pl-3 text-dim ring-1 ring-transparent transition-[box-shadow,background-color] focus-within:bg-panel focus-within:shadow-[0_0_0_4px] focus-within:shadow-focus/15 focus-within:ring-focus/60"
    onclick={() => input.focus()}
  >
    <Search size={15} strokeWidth={2} class="mr-1 shrink-0" />
    {#each tags as tag, index (tag)}
      {@const token = parseTag(tag)}
      <span
        class={[
          "group flex h-6 items-center gap-0.5 rounded-sm pr-0.5 pl-2 text-sm font-medium",
          tone(tag),
          token.excluded && "line-through decoration-1",
        ]}
      >
        {tag}
        <button
          class="grid size-5 place-items-center rounded-sm opacity-60 transition-opacity hover:opacity-100"
          aria-label={m().search.removeTag(tag)}
          onclick={(event) => {
            event.stopPropagation();
            remove(index);
          }}
        >
          <X size={12} strokeWidth={2} />
        </button>
      </span>
    {/each}
    <input
      bind:this={input}
      bind:value={draft}
      {oninput}
      {onkeydown}
      onblur={closeSuggestions}
      type="text"
      spellcheck="false"
      autocomplete="off"
      role="combobox"
      aria-expanded={open}
      aria-controls="tag-suggestions"
      aria-autocomplete="list"
      placeholder={tags.length === 0 ? m().search.placeholder : ""}
      class="h-7 min-w-24 flex-1 bg-transparent text-ink outline-none select-text placeholder:text-dim"
    />
    <kbd class="rounded-sm border border-seam px-1.5 font-sans text-xs text-faint">{shortcutLabel("K")}</kbd>
  </div>

  {#if open}
    <ul
      id="tag-suggestions"
      role="listbox"
      aria-label={m().search.suggestions}
      class="absolute inset-x-0 top-full z-20 mt-1 overflow-hidden rounded-md bg-panel py-1 shadow-panel"
    >
      {#each suggestions as suggestion, index (suggestion.id)}
        <li role="option" aria-selected={index === highlighted}>
          <!-- Pointer-down, not click: a click would blur the field first and close the list. -->
          <button
            class={[
              "flex h-8 w-full items-center gap-2.5 px-3 text-left",
              index === highlighted ? "bg-selected" : "hover:bg-hover",
            ]}
            tabindex="-1"
            onpointerdown={(event) => {
              event.preventDefault();
              accept(suggestion);
            }}
            onpointerenter={() => (highlighted = index)}
          >
            <span class={["size-2 shrink-0 rounded-full", categoryDot[suggestion.category]]}></span>
            <span class="min-w-0 flex-1 truncate">
              {#if suggestion.antecedentName}
                <span class="text-dim">{suggestion.antecedentName} → </span>
              {/if}
              {suggestion.name}
            </span>
            <span class="text-xs text-faint">{suggestion.postCount.toLocaleString(currentLocale())}</span>
          </button>
        </li>
      {/each}
    </ul>
  {/if}
</div>
