<script lang="ts">
  import { currentLocale, m } from "../lib/i18n/index.svelte";
  import type { ListState } from "../lib/lists.svelte";
  import type { SetSummary } from "../lib/sets.svelte";
  import CollectionList from "./CollectionList.svelte";

  let {
    list,
    mine,
    empty,
    onopen,
  }: {
    list: ListState & { readonly items: SetSummary[] };
    /** The user's own list, where the sets they only maintain are marked. */
    mine: boolean;
    empty: string;
    onopen: (id: number) => void;
  } = $props();

  const dateFormat = $derived(new Intl.DateTimeFormat(currentLocale(), { dateStyle: "medium" }));
  const rows = $derived(
    list.items.map((set) => ({
      id: set.id,
      name: set.name,
      cover: set.cover,
      note: !set.isPublic ? m().sets.private : mine && !set.owned ? m().sets.maintained : undefined,
      detail: [`set:${set.shortname}`, m().sets.posts(set.postCount), dateFormat.format(new Date(set.updatedAt))].join(
        " · ",
      ),
    })),
  );
</script>

<CollectionList {list} {rows} {empty} {onopen} />
