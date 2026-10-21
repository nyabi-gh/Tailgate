import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { revealItemInDir } from "@tauri-apps/plugin-opener";
import { account } from "./account.svelte";
import { apiErrorMessage } from "./errors";
import { m } from "./i18n/index.svelte";
import { apiErrorCode, type Favorited, type Post, type Voted } from "./posts";
import { updatePost } from "./search.svelte";
import { notify } from "./toast.svelte";

/** Mirrors `Saved` and `DownloadError` in src-tauri/src/download.rs. */
type Saved = { state: "saved" | "alreadySaved"; path: string };
const downloadErrors = ["notMedia", "network", "http", "storage"] as const;
type DownloadError = (typeof downloadErrors)[number];

let askToSignIn = () => {};

/** Favorites and votes need an account; without one they open the sign-in sheet instead. */
export function onSignInNeeded(handler: () => void) {
  askToSignIn = handler;
}

function signedIn(): boolean {
  if (account().state === "signedIn") return true;
  askToSignIn();
  return false;
}

/** Shown at once, then corrected from e621's reply, or undone if the request fails. */
export async function toggleFavorite(post: Post) {
  if (!signedIn()) return;
  const favorited = !post.isFavorited;
  updatePost(post.id, (current) => ({
    ...current,
    isFavorited: favorited,
    favCount: current.favCount + (favorited ? 1 : -1),
  }));
  try {
    const reply = await invoke<Favorited>("favorite", { id: post.id, favorited });
    updatePost(post.id, (current) => ({ ...current, favCount: reply.favoriteCount }));
  } catch (cause) {
    updatePost(post.id, (current) => ({ ...current, isFavorited: post.isFavorited, favCount: post.favCount }));
    notify(`${m().actions.favoriteFailed} ${apiErrorMessage(apiErrorCode(cause))}`);
  }
}

/** Voting the same way again takes the vote back, as on the site. */
export async function vote(post: Post, up: boolean) {
  if (!signedIn()) return;
  const direction = up ? 1 : -1;
  const next = post.vote === direction ? 0 : direction;
  updatePost(post.id, (current) => ({ ...current, vote: next, score: guessScore(current, next) }));
  try {
    const reply = await invoke<Voted>("vote", { id: post.id, up });
    updatePost(post.id, (current) => ({
      ...current,
      vote: reply.ourScore,
      score: { up: reply.up, down: reply.down, total: reply.score },
    }));
  } catch (cause) {
    updatePost(post.id, (current) => ({ ...current, vote: post.vote, score: post.score }));
    notify(`${m().actions.voteFailed} ${apiErrorMessage(apiErrorCode(cause))}`);
  }
}

/** The score once the user's vote moves from `post.vote` to `next`, until e621 replies. */
function guessScore(post: Post, next: number): Post["score"] {
  const before = post.vote;
  const up = post.score.up - (before === 1 ? 1 : 0) + (next === 1 ? 1 : 0);
  const down = post.score.down + (before === -1 ? 1 : 0) - (next === -1 ? 1 : 0);
  return { up, down, total: up + down };
}

/** Bytes received so far per post being saved, with the total when known. */
let progress = $state<Record<number, { received: number; total: number }>>({});

export function downloadProgress(id: number) {
  return progress[id];
}

export async function watchDownloads() {
  await listen<{ id: number; received: number; total: number }>("download-progress", (event) => {
    const { id, received, total } = event.payload;
    if (id in progress) progress[id] = { received, total };
  });
}

export async function download(post: Post) {
  const url = post.file.url;
  if (url === null || post.id in progress) return;
  progress[post.id] = { received: 0, total: post.file.size };
  try {
    const saved = await invoke<Saved>("download", {
      request: { id: post.id, url, ext: post.file.ext, artists: post.tags.artist },
    });
    const name = saved.path.split(/[\\/]/).pop() ?? saved.path;
    const text = saved.state === "saved" ? m().actions.saved(name) : m().actions.alreadySaved(name);
    notify(text, {
      label: m().actions.reveal,
      run: () => revealItemInDir(saved.path).catch((cause) => console.error("could not show the file", cause)),
    });
  } catch (cause) {
    notify(`${m().actions.downloadFailed} ${downloadErrorMessage(cause)}`);
  } finally {
    delete progress[post.id];
  }
}

function downloadErrorMessage(cause: unknown): string {
  const code = typeof cause === "object" && cause !== null && "code" in cause ? cause.code : undefined;
  const known = downloadErrors.find((candidate) => candidate === code);
  if (known === undefined) throw cause;
  const messages = m().actions.downloadError;
  return known === "storage" || known === "network" ? messages[known] : messages.other;
}
