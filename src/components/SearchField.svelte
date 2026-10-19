<script lang="ts">
  import { Search } from "@lucide/svelte";
  import { m } from "../lib/i18n/index.svelte";
  import { shortcutLabel } from "../lib/platform";

  let { value = $bindable("") }: { value?: string } = $props();

  let input: HTMLInputElement;

  export function focus() {
    input.focus();
  }
</script>

<label
  class="flex h-9 w-full max-w-[640px] items-center gap-2 rounded-m bg-raised px-3 text-dim ring-1 ring-transparent transition-[box-shadow,background-color] focus-within:bg-panel focus-within:shadow-[0_0_0_4px] focus-within:shadow-focus/15 focus-within:ring-focus/60"
>
  <Search size={15} strokeWidth={2} />
  <input
    bind:this={input}
    bind:value
    type="text"
    spellcheck="false"
    autocomplete="off"
    placeholder={m().search.placeholder}
    class="min-w-0 flex-1 bg-transparent text-ink outline-none select-text placeholder:text-dim"
  />
  <kbd class="rounded-s border border-seam px-1.5 font-sans text-xs text-faint">{shortcutLabel("K")}</kbd>
</label>
