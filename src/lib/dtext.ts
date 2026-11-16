/**
 * A reader for the part of e621's DText that comments and descriptions mostly use. Markup
 * outside it stays as written. The result is a tree, never HTML, so nothing a user wrote can
 * become markup of ours.
 */

export type Style = "b" | "i" | "u" | "s" | "sup" | "sub";

export type Inline =
  | { type: "text"; text: string }
  | { type: "break" }
  | { type: "style"; style: Style; children: Inline[] }
  | { type: "spoiler"; children: Inline[] }
  | { type: "code"; text: string }
  | { type: "link"; href: string; text: string }
  /** `{{tags}}`: a post search. */
  | { type: "search"; tags: string[]; text: string };

export type ListItem = { depth: number; children: Inline[] };

export type Block =
  | { type: "paragraph"; children: Inline[] }
  | { type: "heading"; level: number; children: Inline[] }
  | { type: "list"; items: ListItem[] }
  | { type: "quote"; children: Block[] }
  | { type: "spoiler"; children: Block[] }
  | { type: "section"; title: string | null; expanded: boolean; children: Block[] }
  | { type: "code"; text: string };

const SITE = "https://e621.net";

/** `post #12` and the like, and the site path each one names. */
const ID_LINKS: Record<string, string> = {
  post: "posts",
  pool: "pools",
  set: "post_sets",
  user: "users",
  comment: "comments",
  topic: "forum_topics",
  forum: "forum_posts",
};

export function parseDText(source: string): Block[] {
  return blocks(source.replace(/\r\n?/g, "\n"));
}

type Span = { start: number; end: number };

/**
 * Where the tag opened just before `from` closes, counting the same tag opened again inside.
 * `null` if it never does; e621 closes such a tag at the end of the text.
 */
function closing(text: string, name: string, from: number): Span | null {
  const tag = new RegExp(`\\[(/?)${name}(?:[,=][^\\]\\n]*)?\\]`, "gi");
  tag.lastIndex = from;
  let depth = 1;
  for (let match = tag.exec(text); match !== null; match = tag.exec(text)) {
    // Code is literal: an opening tag inside it is text.
    if (name === "code" && match[1] === "") continue;
    depth += match[1] === "/" ? -1 : 1;
    if (depth === 0) return { start: match.index, end: match.index + match[0].length };
  }
  return null;
}

function blocks(text: string): Block[] {
  const open = /\[(?:(quote|code|spoiler)|section(,expanded)?(?:=([^\]\n]*))?)\]/gi;
  const out: Block[] = [];
  let at = 0;
  let scan = 0;
  for (;;) {
    open.lastIndex = scan;
    const match = open.exec(text);
    if (match === null) break;
    const name = match[1]?.toLowerCase() ?? "section";
    const start = match.index + match[0].length;
    const close = closing(text, name, start);
    const inner = text.slice(start, close?.start ?? text.length);
    // A spoiler within one paragraph is part of its line; `inlines` reads it.
    if (name === "spoiler" && !/\n[ \t]*\n/.test(inner)) {
      scan = start;
      continue;
    }
    out.push(...textBlocks(text.slice(at, match.index)));
    if (name === "code") out.push({ type: "code", text: inner.replace(/^\n+|\s+$/g, "") });
    else if (name === "quote") out.push({ type: "quote", children: blocks(inner) });
    else if (name === "spoiler") out.push({ type: "spoiler", children: blocks(inner) });
    else {
      const title = match[3]?.trim();
      out.push({ type: "section", title: title ? title : null, expanded: match[2] !== undefined, children: blocks(inner) });
    }
    at = scan = close?.end ?? text.length;
  }
  out.push(...textBlocks(text.slice(at)));
  return out;
}

/** Paragraphs, headings and lists: the blocks that lines make without tags. */
function textBlocks(text: string): Block[] {
  const out: Block[] = [];
  let paragraph: string[] = [];
  let list: ListItem[] = [];
  const flush = () => {
    if (paragraph.length > 0) out.push({ type: "paragraph", children: inlines(paragraph.join("\n")) });
    if (list.length > 0) out.push({ type: "list", items: list });
    paragraph = [];
    list = [];
  };
  for (const line of text.split("\n")) {
    const [, level, title = ""] = /^h([1-6])\.\s*(.+)$/i.exec(line) ?? [];
    const [, stars, item = ""] = /^(\*+)\s+(.+)$/.exec(line) ?? [];
    if (level !== undefined) {
      flush();
      out.push({ type: "heading", level: Number(level), children: inlines(title) });
    } else if (stars !== undefined) {
      if (paragraph.length > 0) flush();
      list.push({ depth: stars.length, children: inlines(item) });
    } else if (line.trim() === "") {
      flush();
    } else {
      if (list.length > 0) flush();
      paragraph.push(line);
    }
  }
  flush();
  return out;
}

/** A node (or nothing, for markup that is dropped) and where the text goes on after it. */
type Found = { node: Inline | null; end: number };

function inlines(text: string): Inline[] {
  const out: Inline[] = [];
  let plain = "";
  const flush = () => {
    if (plain !== "") out.push({ type: "text", text: plain });
    plain = "";
  };
  let at = 0;
  while (at < text.length) {
    const found = inlineAt(text, at);
    if (found === null) {
      plain += text[at];
      at += 1;
      continue;
    }
    if (found.node !== null) {
      flush();
      out.push(found.node);
    }
    at = found.end;
  }
  flush();
  return out;
}

const STYLES: Record<string, Style | undefined> = { b: "b", i: "i", u: "u", s: "s", sup: "sup", sub: "sub" };
const STYLE_OPEN = /\[(b|i|u|s|sup|sub|spoiler)\]/iy;
const STYLE_CLOSE = /\[\/(b|i|u|s|sup|sub|spoiler)\]/iy;
const WIKI = /\[\[([^\]|\n]+)(?:\|([^\]\n]*))?\]\]/y;
const SEARCH = /\{\{([^}|\n]+)(?:\|([^}\n]*))?\}\}/y;
const CODE = /`([^`\n]+)`/y;
const NAMED_LINK = /"([^"\n]+)":(?:\[([^\]\n]+)\]|(\S+))/y;
const BARE_LINK = /https?:\/\/[^\s<>"]+/iy;
const ID_LINK = /(post|pool|set|user|comment|topic|forum) #(\d+)/iy;

function matchAt(pattern: RegExp, text: string, index: number): RegExpExecArray | null {
  pattern.lastIndex = index;
  return pattern.exec(text);
}

function inlineAt(text: string, index: number): Found | null {
  const char = text[index];
  if (char === "\n") return { node: { type: "break" }, end: index + 1 };
  if (char === "[") return tagAt(text, index);
  if (char === "{") {
    const match = matchAt(SEARCH, text, index);
    if (match === null) return null;
    const [, query = "", label] = match;
    const tags = query.trim().split(/\s+/);
    return { node: { type: "search", tags, text: label?.trim() || tags.join(" ") }, end: SEARCH.lastIndex };
  }
  if (char === "`") {
    const [, code] = matchAt(CODE, text, index) ?? [];
    return code === undefined ? null : { node: { type: "code", text: code }, end: CODE.lastIndex };
  }
  if (char === '"') return namedLinkAt(text, index);
  if (/[\p{L}\p{N}]/u.test(text.charAt(index - 1))) return null;
  if (char === "h" || char === "H") {
    const match = matchAt(BARE_LINK, text, index);
    if (match === null) return null;
    const href = trimLink(match[0]);
    return { node: { type: "link", href, text: href }, end: index + href.length };
  }
  const match = matchAt(ID_LINK, text, index);
  if (match === null) return null;
  const [reference, kind = "", id] = match;
  const href = `${SITE}/${ID_LINKS[kind.toLowerCase()]}/${id}`;
  return { node: { type: "link", href, text: reference }, end: ID_LINK.lastIndex };
}

function tagAt(text: string, index: number): Found | null {
  const [, tag] = matchAt(STYLE_OPEN, text, index) ?? [];
  if (tag !== undefined) {
    const name = tag.toLowerCase();
    const start = STYLE_OPEN.lastIndex;
    const close = closing(text, name, start);
    const children = inlines(text.slice(start, close?.start ?? text.length));
    const style = STYLES[name];
    const node: Inline = style === undefined ? { type: "spoiler", children } : { type: "style", style, children };
    return { node, end: close?.end ?? text.length };
  }
  // A closing tag left over from one that a paragraph or a block cut short.
  if (matchAt(STYLE_CLOSE, text, index) !== null) return { node: null, end: STYLE_CLOSE.lastIndex };
  const wiki = matchAt(WIKI, text, index);
  if (wiki === null) return null;
  const [, name = "", label] = wiki;
  const page = name.trim();
  const title = encodeURIComponent(page.toLowerCase().replace(/ /g, "_"));
  return {
    node: { type: "link", href: `${SITE}/wiki_pages/show_or_new?title=${title}`, text: label?.trim() || page },
    end: WIKI.lastIndex,
  };
}

function namedLinkAt(source: string, index: number): Found | null {
  const match = matchAt(NAMED_LINK, source, index);
  if (match === null) return null;
  // Either the bracketed address matched or the bare one did.
  const [, text = "", bracketed, bare = ""] = match;
  const target = bracketed ?? trimLink(bare);
  const href = /^https?:\/\//i.test(target) ? target : target.startsWith("/") ? SITE + target : null;
  if (href === null) return null;
  const cut = bracketed === undefined ? bare.length - target.length : 0;
  return { node: { type: "link", href, text: text }, end: NAMED_LINK.lastIndex - cut };
}

/** Without the punctuation that ends the sentence a link sits in. */
function trimLink(link: string): string {
  let trimmed = link.replace(/[.,;:!?'"]+$/, "");
  if (trimmed.endsWith(")") && !trimmed.includes("(")) trimmed = trimmed.slice(0, -1);
  return trimmed;
}
