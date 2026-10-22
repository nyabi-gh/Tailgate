<script lang="ts">
  import { ChevronDown, Search } from "@lucide/svelte";
  import { m } from "../lib/i18n/index.svelte";
  import { poolIndex, type PoolCategory, type PoolOrder } from "../lib/pools.svelte";

  const categories: (PoolCategory | null)[] = [null, "series", "collection"];
  const orders: PoolOrder[] = ["updated", "name", "created", "postCount"];

  let name = $state(poolIndex.query.name);

  function submit(event: SubmitEvent) {
    event.preventDefault();
    if (name.trim() !== poolIndex.query.name) poolIndex.search({ name: name.trim() });
  }

  const segment = (selected: boolean) => [
    "h-7 rounded-sm px-3 transition-colors",
    selected ? "bg-panel font-medium text-ink shadow-panel" : "text-ink/75 hover:text-ink",
  ];
</script>

<div class="flex min-w-0 flex-1 items-center gap-3">
  <form class="min-w-0 flex-1" onsubmit={submit}>
    <label
      class="flex h-9 max-w-[360px] items-center gap-2 rounded-md bg-raised px-3 ring-1 ring-transparent transition-[box-shadow,background-color] focus-within:bg-panel focus-within:shadow-[0_0_0_4px] focus-within:shadow-focus/15 focus-within:ring-focus/60"
    >
      <Search size={15} strokeWidth={1.75} class="shrink-0 text-dim" />
      <span class="sr-only">{m().pools.search}</span>
      <input
        class="min-w-0 flex-1 bg-transparent outline-none select-text placeholder:text-faint"
        type="search"
        placeholder={m().pools.search}
        spellcheck="false"
        autocomplete="off"
        bind:value={name}
      />
    </label>
  </form>

  <div class="flex shrink-0 rounded-md bg-hover p-0.5" role="radiogroup" aria-label={m().pools.filter}>
    {#each categories as category (category)}
      {@const selected = poolIndex.query.category === category}
      <button
        class={segment(selected)}
        role="radio"
        aria-checked={selected}
        onclick={() => !selected && poolIndex.search({ category })}
      >
        {m().pools.categories[category ?? "all"]}
      </button>
    {/each}
  </div>

  <label class="relative flex h-9 shrink-0 items-center rounded-md text-ink/80 transition-colors hover:bg-hover">
    <span class="sr-only">{m().pools.sort.label}</span>
    <select
      class="h-full appearance-none bg-transparent pr-7 pl-3 outline-none"
      value={poolIndex.query.order}
      onchange={(event) => {
        const order = orders.find((candidate) => candidate === event.currentTarget.value);
        if (order) poolIndex.search({ order });
      }}
    >
      {#each orders as order (order)}
        <option value={order}>{m().pools.sort[order]}</option>
      {/each}
    </select>
    <ChevronDown size={14} strokeWidth={2} class="pointer-events-none absolute right-2.5 text-dim" />
  </label>
</div>
