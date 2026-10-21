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

/**
 * The posts one screen shows, read page by page from a search Rust keeps open. Each screen has
 * its own, so switching screens keeps the results and the blacklist reveal of each.
 */
export class PostList {
  #id: number | null = null;
  /** Bumped by every start and clear, so a reply for an older one is dropped. */
  #generation = 0;
  /** What was asked for: the tags for a search, `""` for the favorites; `null` before any. */
  query = $state<string | null>(null);
  // Replaced, never mutated: a deep proxy over thousands of posts would only cost.
  #posts = $state.raw<Post[]>([]);
  #blacklisted = $state.raw(new Set<number>());
  revealed = $state(false);
  done = $state(false);
  loading = $state(false);
  error = $state<ApiErrorCode | null>(null);
  #shown = $derived(this.revealed ? this.#posts : this.#posts.filter((post) => !this.#blacklisted.has(post.id)));

  /** Blacklisted posts left out unless revealed. */
  get posts() {
    return this.#shown;
  }

  get hiddenCount() {
    return this.#blacklisted.size;
  }

  isBlacklisted(post: Post) {
    return this.#blacklisted.has(post.id);
  }

  /**
   * Rust closes the screen's previous search when one starts, so nothing is awaited before
   * `open`: the starts reach Rust in the order they were made, and the newest one stays open.
   */
  async start(query: string, open: () => Promise<number>) {
    const mine = ++this.#generation;
    this.#reset(query);
    const started = await open();
    if (mine !== this.#generation) return;
    this.#id = started;
    await this.loadMore();
  }

  /**
   * Asks for the next page unless one is already on its way, the search is done or failed, or
   * the connection has no route yet (the grid asks again once it has).
   */
  async loadMore() {
    const current = this.#id;
    if (current === null || this.loading || this.done || this.error !== null || !canLoadRemote()) return;
    this.loading = true;
    try {
      const page = await invoke<SearchPage>("search_next", { id: current });
      if (current !== this.#id) return;
      this.#posts = [...this.#posts, ...page.posts];
      if (page.blacklisted.length > 0) this.#blacklisted = new Set([...this.#blacklisted, ...page.blacklisted]);
      this.done = page.done;
      rememberCategories(page.categories);
    } catch (cause) {
      if (current !== this.#id) return;
      this.error = apiErrorCode(cause);
    } finally {
      if (current === this.#id) this.loading = false;
    }
  }

  async retry() {
    this.error = null;
    await this.loadMore();
  }

  /** Shows or hides again the blacklisted posts; a new search hides them. */
  toggleRevealed() {
    this.revealed = !this.revealed;
  }

  update(id: number, change: (post: Post) => Post) {
    if (this.#posts.some((post) => post.id === id)) {
      this.#posts = this.#posts.map((post) => (post.id === id ? change(post) : post));
    }
  }

  #reset(query: string | null) {
    this.#id = null;
    this.query = query;
    this.#posts = [];
    this.#blacklisted = new Set();
    this.revealed = false;
    this.done = false;
    this.error = null;
    this.loading = false;
  }
}

export const tagSearch = new PostList();
export const favorites = new PostList();

export function startSearch(tags: string) {
  return tagSearch.start(tags, () => invoke<number>("search_start", { tags }));
}

export function startFavorites() {
  return favorites.start("", () => invoke<number>("search_start_favorites"));
}

/** A change to a post (a favorite, a vote) shows on every screen that lists it. */
export function updatePost(id: number, change: (post: Post) => Post) {
  tagSearch.update(id, change);
  favorites.update(id, change);
}

let categories = $state<Record<string, TagCategory>>({});

/** The category a tag is known to be in, from suggestions and search results seen so far. */
export function knownCategory(name: string): TagCategory | undefined {
  return categories[name.toLowerCase()];
}

export function rememberCategories(found: Record<string, TagCategory>) {
  for (const [name, category] of Object.entries(found)) categories[name.toLowerCase()] = category;
}
