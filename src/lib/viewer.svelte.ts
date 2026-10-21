import type { PostList } from "./search.svelte";

/** From this close to the end of what has arrived, the next page is asked for. */
const LOAD_AHEAD = 5;

let openId = $state<number | null>(null);
/** The screen's posts the viewer was opened from, and steps through. */
let list = $state.raw<PostList | null>(null);
let panelOpen = $state(true);
// Kept for the run, so stepping through videos does not start each one loud.
let muted = $state(true);

/** Position in the grid's posts, which leave out hidden blacklisted ones. */
const posts = $derived(list?.posts ?? []);
const index = $derived(openId === null ? -1 : posts.findIndex((post) => post.id === openId));

export const viewer = {
  get open() {
    return openId !== null;
  },
  get post() {
    return index >= 0 ? posts[index] : undefined;
  },
  get index() {
    return index;
  },
  get hasPrevious() {
    return index > 0;
  },
  get hasNext() {
    return index >= 0 && index < posts.length - 1;
  },
  get panelOpen() {
    return panelOpen;
  },
  get muted() {
    return muted;
  },
  get posts() {
    return posts;
  },
};

export function openPost(from: PostList, id: number) {
  list = from;
  openId = id;
  ensureAhead();
}

export function closeViewer() {
  openId = null;
}

export function step(by: 1 | -1) {
  const next = posts[index + by];
  if (next === undefined) return;
  openId = next.id;
  ensureAhead();
}

export function togglePanel() {
  panelOpen = !panelOpen;
}

export function setMuted(value: boolean) {
  muted = value;
}

function ensureAhead() {
  const at = posts.findIndex((post) => post.id === openId);
  if (at >= posts.length - LOAD_AHEAD) list?.loadMore();
}
