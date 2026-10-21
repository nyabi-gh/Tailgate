import type { TagCategory } from "./posts";

/**
 * Common e621 search metatags. A `name:value` token not listed here is treated as a tag, which
 * only affects how it is colored.
 */
const METATAGS = new Set([
  "order",
  "rating",
  "type",
  "status",
  "score",
  "favcount",
  "id",
  "width",
  "height",
  "filesize",
  "date",
  "tagcount",
  "user",
  "fav",
  "pool",
  "set",
  "source",
  "md5",
  "duration",
  "ratio",
  "limit",
  "randseed",
  "parent",
  "child",
  "approver",
  "commenter",
  "noter",
  "description",
  "ischild",
  "isparent",
  "inpool",
  "pending_replacements",
  "artverified",
]);

export type TagToken = {
  /** As typed, prefix included. */
  text: string;
  /** Without the `-` (exclude) or `~` (any of) prefix. */
  name: string;
  excluded: boolean;
  metatag: boolean;
};

export function parseTag(text: string): TagToken {
  const prefix = text.match(/^[-~]/)?.[0] ?? "";
  const name = text.slice(prefix.length);
  const colon = name.indexOf(":");
  return {
    text,
    name,
    excluded: prefix === "-",
    metatag: colon > 0 && METATAGS.has(name.slice(0, colon).toLowerCase()),
  };
}

export function splitTags(query: string): string[] {
  return query.split(/\s+/).filter(Boolean);
}

/** Text and background classes per category, spelled out so Tailwind finds them. */
export const categoryTone: Record<TagCategory | "unknown", string> = {
  general: "text-tag-general bg-tag-general/12",
  artist: "text-tag-artist bg-tag-artist/12",
  contributor: "text-tag-artist bg-tag-artist/12",
  copyright: "text-tag-copyright bg-tag-copyright/12",
  character: "text-tag-character bg-tag-character/12",
  species: "text-tag-species bg-tag-species/12",
  invalid: "text-tag-invalid bg-tag-invalid/12",
  meta: "text-tag-meta bg-tag-meta/12",
  lore: "text-tag-lore bg-tag-lore/12",
  unknown: "text-ink bg-selected",
};

export const categoryDot: Record<TagCategory, string> = {
  general: "bg-tag-general",
  artist: "bg-tag-artist",
  contributor: "bg-tag-artist",
  copyright: "bg-tag-copyright",
  character: "bg-tag-character",
  species: "bg-tag-species",
  invalid: "bg-tag-invalid",
  meta: "bg-tag-meta",
  lore: "bg-tag-lore",
};
