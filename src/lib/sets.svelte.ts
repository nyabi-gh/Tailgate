import { invoke } from "@tauri-apps/api/core";
import { PagedList } from "./lists.svelte";
import { setPosts } from "./search.svelte";

/** Mirrors `SetOrder` in src-tauri/src/api/endpoints.rs. */
export type SetOrder = "updated" | "name" | "postCount" | "created";
/** Mirrors `SetQuery` in src-tauri/src/api/endpoints.rs. */
export type SetQuery = { name: string; order: SetOrder };
/** Mirrors `PostSet` in src-tauri/src/api/models.rs. */
export type PostSet = { id: number; name: string; shortname: string; isPublic: boolean; updatedAt: string };
/** Mirrors `SetSummary` in src-tauri/src/search.rs. */
export type SetSummary = PostSet & { postCount: number; cover: string | null; owned: boolean };
type SetPage = { sets: SetSummary[]; done: boolean };
/** Mirrors `OpenedSet` in src-tauri/src/search.rs. */
type OpenedSet = { search: number; set: PostSet; postCount: number };

/** Which list the set screen shows. */
export const setMode = $state<{ mode: "mine" | "search" }>({ mode: "mine" });

/** The signed-in user's sets, all in one reply. */
export const mySets = new PagedList<Record<string, never>, SetSummary>({}, async () => ({
  items: await invoke<SetSummary[]>("my_sets"),
  done: true,
}));

export const setIndex = new PagedList<SetQuery, SetSummary>({ name: "", order: "updated" }, async (query, page) => {
  const reply = await invoke<SetPage>("sets_page", { query, page });
  return { items: reply.sets, done: reply.done };
});

let mineStale = $state(false);

/** Set when the user's sets changed elsewhere (a post added, another account), so they are read again. */
export function markMySetsStale() {
  mineStale = true;
}

export function mySetsStale(): boolean {
  return mineStale;
}

export function reloadMySets() {
  mineStale = false;
  mySets.search();
}

/** The set being looked at, or `null` while a list shows; its posts are in `setPosts`. */
let opened = $state<{ id: number; set: PostSet | null; postCount: number | null } | null>(null);

export function openedSet() {
  return opened;
}

export function openSet(id: number) {
  opened = { id, set: null, postCount: null };
  return setPosts.start(`set ${id}`, async () => {
    const reply = await invoke<OpenedSet>("search_start_set", { id });
    if (opened?.id === id) {
      opened = { id, set: reply.set, postCount: reply.postCount };
      setPosts.total = reply.postCount;
    }
    return reply.search;
  });
}

/** Back to the list, which kept its place. */
export function closeSet() {
  opened = null;
}
