import { invoke } from "@tauri-apps/api/core";
import { PagedList } from "./lists.svelte";
import { apiErrorCode } from "./posts";
import { poolPosts } from "./search.svelte";

/** Mirrors `PoolCategory` in src-tauri/src/api/models.rs. */
export type PoolCategory = "series" | "collection";
/** Mirrors `PoolOrder` in src-tauri/src/api/endpoints.rs. */
export type PoolOrder = "updated" | "name" | "created" | "postCount";
/** Mirrors `PoolQuery` in src-tauri/src/api/endpoints.rs. */
export type PoolQuery = { name: string; category: PoolCategory | null; order: PoolOrder };
/** Mirrors `Pool` in src-tauri/src/api/models.rs. */
export type Pool = {
  id: number;
  name: string;
  category: PoolCategory;
  creator: string | null;
  updatedAt: string;
};
/** Mirrors `PoolSummary` in src-tauri/src/search.rs. */
export type PoolSummary = Pool & { postCount: number; cover: string | null };
type PoolPage = { pools: PoolSummary[]; done: boolean };
/** Mirrors `OpenedPool` in src-tauri/src/search.rs. */
type OpenedPool = { search: number; pool: Pool; postCount: number };

/** The pool list for the current query. */
export const poolIndex = new PagedList<PoolQuery, PoolSummary>(
  { name: "", category: null, order: "updated" },
  async (query, page) => {
    const reply = await invoke<PoolPage>("pools_page", { query, page });
    return { items: reply.pools, done: reply.done };
  },
);

/** The pool being read, or `null` while the list shows; its posts are in `poolPosts`. */
let opened = $state<{ id: number; pool: Pool | null; postCount: number | null } | null>(null);

export function openedPool() {
  return opened;
}

export function openPool(id: number) {
  opened = { id, pool: null, postCount: null };
  return poolPosts.start(`pool ${id}`, async () => {
    const reply = await invoke<OpenedPool>("search_start_pool", { id });
    if (opened?.id === id) {
      opened = { id, pool: reply.pool, postCount: reply.postCount };
      poolPosts.total = reply.postCount;
    }
    return reply.search;
  });
}

/** Back to the list, which kept its place. */
export function closePool() {
  opened = null;
}

let names = $state<Record<number, string>>({});
const asked = new Set<number>();

/** A pool's name once known; the info panel asks for the names it shows. */
export function poolName(id: number): string | undefined {
  return names[id];
}

export async function learnPoolNames(ids: number[]) {
  const missing = ids.filter((id) => names[id] === undefined && !asked.has(id));
  if (missing.length === 0) return;
  for (const id of missing) asked.add(id);
  try {
    const pools = await invoke<Pool[]>("pools_by_id", { ids: missing });
    names = { ...names, ...Object.fromEntries(pools.map((pool) => [pool.id, pool.name])) };
  } catch (cause) {
    // The panel shows the pool's number instead; the next post that lists it asks again.
    for (const id of missing) asked.delete(id);
    console.error("could not get pool names", apiErrorCode(cause));
  }
}
