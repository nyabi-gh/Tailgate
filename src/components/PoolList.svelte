<script lang="ts">
  import { currentLocale, m } from "../lib/i18n/index.svelte";
  import { poolIndex } from "../lib/pools.svelte";
  import CollectionList from "./CollectionList.svelte";

  let { onopen }: { onopen: (id: number) => void } = $props();

  const dateFormat = $derived(new Intl.DateTimeFormat(currentLocale(), { dateStyle: "medium" }));
  const rows = $derived(
    poolIndex.items.map((pool) => ({
      id: pool.id,
      name: pool.name,
      cover: pool.cover,
      detail: [
        m().pools.category[pool.category],
        m().pools.posts(pool.postCount),
        dateFormat.format(new Date(pool.updatedAt)),
        pool.creator === null ? null : m().pools.by(pool.creator),
      ]
        .filter((part) => part !== null)
        .join(" · "),
    })),
  );
</script>

<CollectionList list={poolIndex} {rows} empty={m().pools.empty} {onopen} />
