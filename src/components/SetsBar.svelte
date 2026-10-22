<script lang="ts">
  import { ChevronDown, Search } from "@lucide/svelte";
  import { m } from "../lib/i18n/index.svelte";
  import { setIndex, setMode, type SetOrder } from "../lib/sets.svelte";

  const modes = ["mine", "search"] as const;
  const orders: SetOrder[] = ["updated", "name", "postCount", "created"];

  let name = $state(setIndex.query.name);

  function submit(event: SubmitEvent) {
    event.preventDefault();
    if (name.trim() !== setIndex.query.name) setIndex.search({ name: name.trim() });
  }

  const segment = (selected: boolean) => [
    "h-7 rounded-sm px-3 transition-colors",
    selected ? "bg-panel font-medium text-ink shadow-panel" : "text-ink/75 hover:text-ink",
  ];
</script>

<div class="flex min-w-0 flex-1 items-center gap-3">
  <div class="flex shrink-0 rounded-md bg-hover p-0.5" role="radiogroup" aria-label={m().sets.show}>
    {#each modes as mode (mode)}
      {@const selected = setMode.mode === mode}
      <button class={segment(selected)} role="radio" aria-checked={selected} onclick={() => (setMode.mode = mode)}>
        {m().sets.modes[mode]}
      </button>
    {/each}
  </div>

  {#if setMode.mode === "search"}
    <form class="min-w-0 flex-1" onsubmit={submit}>
      <label
        class="flex h-9 max-w-[360px] items-center gap-2 rounded-md bg-raised px-3 ring-1 ring-transparent transition-[box-shadow,background-color] focus-within:bg-panel focus-within:shadow-[0_0_0_4px] focus-within:shadow-focus/15 focus-within:ring-focus/60"
      >
        <Search size={15} strokeWidth={1.75} class="shrink-0 text-dim" />
        <span class="sr-only">{m().sets.search}</span>
        <input
          class="min-w-0 flex-1 bg-transparent outline-none select-text placeholder:text-faint"
          type="search"
          placeholder={m().sets.search}
          spellcheck="false"
          autocomplete="off"
          bind:value={name}
        />
      </label>
    </form>

    <label class="relative flex h-9 shrink-0 items-center rounded-md text-ink/80 transition-colors hover:bg-hover">
      <span class="sr-only">{m().sets.sort.label}</span>
      <select
        class="h-full appearance-none bg-transparent pr-7 pl-3 outline-none"
        value={setIndex.query.order}
        onchange={(event) => {
          const order = orders.find((candidate) => candidate === event.currentTarget.value);
          if (order) setIndex.search({ order });
        }}
      >
        {#each orders as order (order)}
          <option value={order}>{m().sets.sort[order]}</option>
        {/each}
      </select>
      <ChevronDown size={14} strokeWidth={2} class="pointer-events-none absolute right-2.5 text-dim" />
    </label>
  {/if}
</div>
