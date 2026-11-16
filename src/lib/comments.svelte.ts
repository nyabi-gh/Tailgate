import { invoke } from "@tauri-apps/api/core";
import { apiErrorCode, type ApiErrorCode } from "./posts";

/** Mirrors `Comment` in src-tauri/src/api/models.rs. */
export type Comment = {
  id: number;
  createdAt: string;
  creator: string | null;
  body: string;
  score: number;
  isSticky: boolean;
};

/** Mirrors `CommentPage` in src-tauri/src/api/models.rs. */
type CommentPage = { comments: Comment[]; done: boolean };

/** One post's comments, oldest first, read page by page. */
export class CommentList {
  readonly post: number;
  items = $state.raw<Comment[]>([]);
  done = $state(false);
  loading = $state(false);
  error = $state<ApiErrorCode | null>(null);
  #page = 0;

  constructor(post: number) {
    this.post = post;
  }

  /** The next page, or the one that failed. */
  async loadMore() {
    if (this.loading || this.done) return;
    this.loading = true;
    this.error = null;
    try {
      const reply = await invoke<CommentPage>("comments", { post: this.post, page: this.#page + 1 });
      this.#page += 1;
      this.items = [...this.items, ...reply.comments];
      this.done = reply.done;
    } catch (cause) {
      this.error = apiErrorCode(cause);
    } finally {
      this.loading = false;
    }
  }
}
