import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { BlacklistMarks } from "./blacklistMarks";
import { canLoadRemote } from "./connection.svelte";
import { dateParam, today, type Day, type Scale } from "./popular";
import { apiErrorCode, type ApiErrorCode, type Post, type TagCategory } from "./posts";

/** Mirrors `SearchPage` in src-tauri/src/search.rs. */
type SearchPage = {
  posts: Post[];
  blacklisted: number[];
  blacklistGeneration: number;
  done: boolean;
  categories: Record<string, TagCategory>;
};

/** Mirrors `BlacklistChange` in src-tauri/src/search.rs. */
type BlacklistChange = {
  generation: number;
  searches: { id: number; blacklisted: number[] }[];
};

/**
 * The posts one screen shows, read page by page from a search Rust keeps open. Each screen has
 * its own, so switching screens keeps the results and the blacklist reveal of each.
 */
export class PostList {
  #id: number | null = null;
  #open: (() => Promise<number>) | null = null;
  /** Bumped by every start and clear, so a reply for an older one is dropped. */
  #generation = 0;
  /**
   * What was asked for: the tags for a search, `""` for the favorites, the period for the popular
   * posts; `null` before any.
   */
  query = $state<string | null>(null);
  // Replaced, never mutated: a deep proxy over thousands of posts would only cost.
  #posts = $state.raw<Post[]>([]);
  #marks = new BlacklistMarks();
  #blacklisted = $state.raw(this.#marks.ids);
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
    this.#open = open;
    let started: number;
    try {
      started = await open();
    } catch (cause) {
      // Opening asks e621 itself for some lists (a pool's posts).
      if (mine === this.#generation) this.error = apiErrorCode(cause);
      return;
    }
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
      if (this.#marks.addPage(page.blacklistGeneration, page.blacklisted)) this.#blacklisted = this.#marks.ids;
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
    if (this.#id === null && this.#open !== null && this.query !== null) {
      await this.start(this.query, this.#open);
      return;
    }
    this.error = null;
    await this.loadMore();
  }

  /** Shows or hides again the blacklisted posts; a new search hides them. */
  toggleRevealed() {
    this.revealed = !this.revealed;
  }

  /** A refreshed blacklist, applied to the posts already loaded. */
  applyBlacklist(change: BlacklistChange) {
    const mine = change.searches.find((search) => search.id === this.#id);
    if (mine !== undefined && this.#marks.applyRefresh(change.generation, mine.blacklisted)) {
      this.#blacklisted = this.#marks.ids;
    }
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
    this.#marks = new BlacklistMarks();
    this.#blacklisted = this.#marks.ids;
    this.revealed = false;
    this.done = false;
    this.error = null;
    this.loading = false;
  }
}

export const tagSearch = new PostList();
export const favorites = new PostList();
export const popular = new PostList();

/** The popular period on screen, kept while other screens are open. */
export const popularPeriod = $state<{ day: Day; scale: Scale }>({ day: today(), scale: "day" });

/** Read in the computer's time zone, so "today" is the user's today rather than e621's. */
export function startPopular() {
  const { day, scale } = popularPeriod;
  const date = dateParam(day);
  const timeZone = Intl.DateTimeFormat().resolvedOptions().timeZone;
  return popular.start(`${scale} ${date}`, () =>
    invoke<number>("search_start_popular", { popular: { date, scale, timeZone } }),
  );
}

export function startSearch(tags: string) {
  return tagSearch.start(tags, () => invoke<number>("search_start", { tags }));
}

export function startFavorites() {
  return favorites.start("", () => invoke<number>("search_start_favorites"));
}

/** Rust asks e621 for the blacklist again when the window comes back to the front. */
export async function watchBlacklist() {
  await listen<BlacklistChange>("blacklist-changed", ({ payload }) => {
    tagSearch.applyBlacklist(payload);
    favorites.applyBlacklist(payload);
    popular.applyBlacklist(payload);
  });
}

/** A change to a post (a favorite, a vote) shows on every screen that lists it. */
export function updatePost(id: number, change: (post: Post) => Post) {
  tagSearch.update(id, change);
  favorites.update(id, change);
  popular.update(id, change);
}

let categories = $state<Record<string, TagCategory>>({});

/** The category a tag is known to be in, from suggestions and search results seen so far. */
export function knownCategory(name: string): TagCategory | undefined {
  return categories[name.toLowerCase()];
}

export function rememberCategories(found: Record<string, TagCategory>) {
  for (const [name, category] of Object.entries(found)) categories[name.toLowerCase()] = category;
}
