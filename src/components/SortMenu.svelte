<script lang="ts">
  import { ChevronDown } from "@lucide/svelte";
  import { m } from "../lib/i18n/index.svelte";
  import { parseTag } from "../lib/tags";

  let { tags, onchange }: { tags: string[]; onchange: (tags: string[]) => void } = $props();

  const sorts = ["latest", "score", "favcount", "oldest", "random"] as const;
  type Sort = (typeof sorts)[number];
  const orders: Record<Sort, string | null> = {
    latest: null,
    score: "order:score",
    favcount: "order:favcount",
    oldest: "order:id",
    random: "order:random",
  };

  const isOrder = (tag: string) => {
    const token = parseTag(tag);
    return token.metatag && !token.excluded && token.name.toLowerCase().startsWith("order:");
  };
  const typed = $derived(tags.find(isOrder));
  const selected = $derived(
    typed === undefined ? "latest" : (sorts.find((sort) => orders[sort] === typed.toLowerCase()) ?? typed),
  );

  function choose(value: string) {
    // The only other option is the typed order, shown disabled.
    const sort = sorts.find((candidate) => candidate === value);
    if (sort === undefined) return;
    const order = orders[sort];
    const rest = tags.filter((tag) => !isOrder(tag));
    onchange(order ? [...rest, order] : rest);
  }
</script>

<label class="relative flex h-9 shrink-0 items-center rounded-md text-ink/80 transition-colors hover:bg-hover">
  <span class="sr-only">{m().search.sort.label}</span>
  <select
    class="h-full appearance-none bg-transparent pr-7 pl-3 outline-none"
    value={selected}
    onchange={(event) => choose(event.currentTarget.value)}
  >
    {#each sorts as sort (sort)}
      <option value={sort}>{m().search.sort[sort]}</option>
    {/each}
    {#if typed !== undefined && !sorts.some((sort) => orders[sort] === typed.toLowerCase())}
      <option value={typed} disabled>{typed}</option>
    {/if}
  </select>
  <ChevronDown size={14} strokeWidth={2} class="pointer-events-none absolute right-2.5 text-dim" />
</label>
