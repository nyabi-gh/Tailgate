import { canLoadRemote } from "./connection.svelte";
import { apiErrorCode, type ApiErrorCode } from "./posts";

export type ListPage<T> = { items: T[]; done: boolean };

/** What a list screen needs from its list, whatever the items are. */
export type ListState = {
  readonly error: ApiErrorCode | null;
  readonly done: boolean;
  readonly loading: boolean;
  loadMore(): void;
  retry(): void;
};

/** A list of pools or sets for a query, read page by page like the grids. */
export class PagedList<Query, Item> {
  query: Query;
  items = $state.raw<Item[]>([]);
  started = $state(false);
  done = $state(false);
  loading = $state(false);
  error = $state<ApiErrorCode | null>(null);
  #fetch: (query: Query, page: number) => Promise<ListPage<Item>>;
  #page = 0;
  /** Bumped by every new query, so a reply for an older one is dropped. */
  #generation = 0;

  constructor(query: Query, fetch: (query: Query, page: number) => Promise<ListPage<Item>>) {
    this.query = $state(query);
    this.#fetch = fetch;
  }

  /** Starts over, with the query changed by `change` if given. */
  search(change: Partial<Query> = {}) {
    this.query = { ...this.query, ...change };
    this.#generation += 1;
    this.#page = 0;
    this.items = [];
    this.started = true;
    this.done = false;
    this.loading = false;
    this.error = null;
    this.loadMore();
  }

  /** Waits for a route, a page in flight, or a retry after an error. */
  async loadMore() {
    if (!this.started || this.loading || this.done || this.error !== null || !canLoadRemote()) return;
    const mine = this.#generation;
    const page = this.#page + 1;
    this.loading = true;
    try {
      const reply = await this.#fetch(this.query, page);
      if (mine !== this.#generation) return;
      this.#page = page;
      this.items = [...this.items, ...reply.items];
      this.done = reply.done;
    } catch (cause) {
      if (mine !== this.#generation) return;
      this.error = apiErrorCode(cause);
    } finally {
      if (mine === this.#generation) this.loading = false;
    }
  }

  retry() {
    this.error = null;
    this.loadMore();
  }
}
