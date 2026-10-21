/** Mirrors src-tauri/src/api/models.rs. */

export type TagCategory =
  | "general"
  | "artist"
  | "contributor"
  | "copyright"
  | "character"
  | "species"
  | "invalid"
  | "meta"
  | "lore";

export type Image = { width: number; height: number; url: string | null };

export type VideoSource = { url: string; width: number; height: number; size: number; codec: string | null };

export type Post = {
  id: number;
  createdAt: string;
  rating: "s" | "q" | "e";
  score: { up: number; down: number; total: number };
  favCount: number;
  isFavorited: boolean;
  vote: number;
  commentCount: number;
  deleted: boolean;
  pending: boolean;
  flagged: boolean;
  description: string;
  sources: string[];
  uploader: { id: number; name: string | null };
  pools: number[];
  parentId: number | null;
  children: number[];
  duration: number | null;
  tags: Record<TagCategory, string[]>;
  file: { ext: string; width: number; height: number; size: number; md5: string; url: string | null };
  preview: Image;
  sample: Image | null;
  video: {
    p720: VideoSource | null;
    p480: VideoSource | null;
    mp4: VideoSource | null;
    original: VideoSource | null;
  } | null;
};

export type TagSuggestion = {
  id: number;
  name: string;
  postCount: number;
  category: TagCategory;
  antecedentName: string | null;
};

/** Mirrors `ApiError` in src-tauri/src/api/mod.rs, plus `closed` from the search commands. */
export const apiErrors = [
  "unauthorized",
  "forbidden",
  "notFound",
  "pageLimit",
  "rateLimited",
  "unavailable",
  "network",
  "decode",
  "http",
  "closed",
] as const;

export type ApiErrorCode = (typeof apiErrors)[number];

/** Commands reject with `{ code }`; anything else is a bug and is rethrown. */
export function apiErrorCode(error: unknown): ApiErrorCode {
  const code = typeof error === "object" && error !== null && "code" in error ? error.code : undefined;
  const known = apiErrors.find((candidate) => candidate === code);
  if (known === undefined) throw error;
  return known;
}
