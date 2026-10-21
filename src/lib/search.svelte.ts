import { invoke } from "@tauri-apps/api/core";
import { canLoadRemote } from "./connection.svelte";
import { apiErrorCode, type ApiErrorCode, type Post, type TagCategory } from "./posts";

/** Mirrors `SearchPage` in src-tauri/src/search.rs. */
type SearchPage = {
  posts: Post[];
  blacklisted: number[];
  done: boolean;
  categories: Record<string, TagCategory>;
};

let id: number | null = null;
/** Bumped by every start and clear, so a reply for an older one is dropped. */
let generation = 0;
let query = $state<string | null>(null);
// Replaced, never mutated: a deep proxy over thousands of posts would only cost.
let posts = $state.raw<Post[]>([]);
let blacklisted = $state.raw(new Set<number>());
let revealed = $state(false);
const shown = $derived(revealed ? posts : posts.filter((post) => !blacklisted.has(post.id)));
let done = $state(false);
let loading = $state(false);
let error = $state<ApiErrorCode | null>(null);
let categories = $state<Record<string, TagCategory>>({});

export const search = {
  get query() {
    return query;
  },
  /** Blacklisted posts left out unless revealed. */
  get posts() {
    return shown;
  },
  get hiddenCount() {
    return blacklisted.size;
  },
  get revealed() {
    return revealed;
  },
  isBlacklisted(post: Post) {
    return blacklisted.has(post.id);
  },
  get done() {
    return done;
  },
  get loading() {
    return loading;
  },
  get error() {
    return error;
  },
};

/** The category a tag is known to be in, from suggestions and search results seen so far. */
export function knownCategory(name: string): TagCategory | undefined {
  return categories[name.toLowerCase()];
}

export function rememberCategories(found: Record<string, TagCategory>) {
  for (const [name, category] of Object.entries(found)) categories[name.toLowerCase()] = category;
}

/**
 * Rust closes the other searches when one starts, so nothing is awaited before `search_start`:
 * the starts reach Rust in the order they were made, and the newest one stays open.
 */
export async function startSearch(tags: string) {
  const mine = ++generation;
  id = null;
  query = tags;
  posts = [];
  blacklisted = new Set();
  revealed = false;
  done = false;
  error = null;
  loading = false;
  const started = await invoke<number>("search_start", { tags });
  if (mine !== generation) return;
  id = started;
  await loadMore();
}

/** Leaves the search screen empty. */
export async function clearSearch() {
  generation++;
  const previous = id;
  id = null;
  query = null;
  posts = [];
  blacklisted = new Set();
  revealed = false;
  done = false;
  error = null;
  loading = false;
  if (previous !== null) await invoke("search_close", { id: previous });
}

/**
 * Asks for the next page unless one is already on its way, the search is done or failed, or the
 * connection has no route yet (the grid asks again once it has).
 */
export async function loadMore() {
  const current = id;
  if (current === null || loading || done || error !== null || !canLoadRemote()) return;
  loading = true;
  try {
    const page = await invoke<SearchPage>("search_next", { id: current });
    if (current !== id) return;
    posts = [...posts, ...page.posts];
    if (page.blacklisted.length > 0) blacklisted = new Set([...blacklisted, ...page.blacklisted]);
    done = page.done;
    rememberCategories(page.categories);
  } catch (cause) {
    if (current !== id) return;
    error = apiErrorCode(cause);
  } finally {
    if (current === id) loading = false;
  }
}

/** Shows or hides again the blacklisted posts of this search; a new search hides them. */
export function toggleRevealed() {
  revealed = !revealed;
}

export async function retry() {
  error = null;
  await loadMore();
}
