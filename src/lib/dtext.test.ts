import { describe, expect, it } from "vitest";
import { parseDText, type Inline } from "./dtext";

const text = (value: string): Inline => ({ type: "text", text: value });

function paragraph(source: string): Inline[] {
  const blocks = parseDText(source);
  expect(blocks).toHaveLength(1);
  const [block] = blocks;
  if (block?.type !== "paragraph") throw new Error(`expected a paragraph, got ${block?.type}`);
  return block.children;
}

describe("inline markup", () => {
  it("nests styles", () => {
    expect(paragraph("a [b]bold [i]both[/i][/b] z")).toEqual([
      text("a "),
      { type: "style", style: "b", children: [text("bold "), { type: "style", style: "i", children: [text("both")] }] },
      text(" z"),
    ]);
  });

  it("reads tags in any case and keeps line breaks", () => {
    expect(paragraph("[U]one[/u]\ntwo")).toEqual([
      { type: "style", style: "u", children: [text("one")] },
      { type: "break" },
      text("two"),
    ]);
  });

  it("closes an unclosed tag at the end of its paragraph and drops the stray closer", () => {
    expect(parseDText("[b]one\n\ntwo[/b]")).toEqual([
      { type: "paragraph", children: [{ type: "style", style: "b", children: [text("one")] }] },
      { type: "paragraph", children: [text("two")] },
    ]);
  });

  it("leaves markup it does not know as written", () => {
    expect(paragraph("[color=red]x[/color] [b")).toEqual([text("[color=red]x[/color] [b")]);
  });

  it("keeps a spoiler within a line inline", () => {
    expect(paragraph("so [spoiler]it [b]dies[/b][/spoiler].")).toEqual([
      text("so "),
      { type: "spoiler", children: [text("it "), { type: "style", style: "b", children: [text("dies")] }] },
      text("."),
    ]);
  });

  it("reads inline code literally", () => {
    expect(paragraph("use `[b]x[/b]` here")).toEqual([text("use "), { type: "code", text: "[b]x[/b]" }, text(" here")]);
  });
});

describe("links", () => {
  it("links bare addresses without the sentence's punctuation", () => {
    expect(paragraph("see https://example.com/a?b=1, or (http://x.org/y).")).toEqual([
      text("see "),
      { type: "link", href: "https://example.com/a?b=1", text: "https://example.com/a?b=1" },
      text(", or ("),
      { type: "link", href: "http://x.org/y", text: "http://x.org/y" },
      text(")."),
    ]);
  });

  it("keeps a closing parenthesis that belongs to the address", () => {
    expect(paragraph("https://en.wikipedia.org/wiki/Wolf_(animal)")).toEqual([
      {
        type: "link",
        href: "https://en.wikipedia.org/wiki/Wolf_(animal)",
        text: "https://en.wikipedia.org/wiki/Wolf_(animal)",
      },
    ]);
  });

  it("reads named links, to the site when the address is a path", () => {
    expect(paragraph('"watchuontheloo":/users/1603546 said: "docs":[https://example.com/a b].')).toEqual([
      { type: "link", href: "https://e621.net/users/1603546", text: "watchuontheloo" },
      text(" said: "),
      { type: "link", href: "https://example.com/a b", text: "docs" },
      text("."),
    ]);
  });

  it("does not link addresses that are not web links", () => {
    const source = '"click":javascript:alert(1) and "x":#anchor';
    expect(paragraph(source)).toEqual([text(source)]);
  });

  it("links numbered references, but not inside a word", () => {
    expect(paragraph("Post #12, compost #3, set #4")).toEqual([
      { type: "link", href: "https://e621.net/posts/12", text: "Post #12" },
      text(", compost #3, "),
      { type: "link", href: "https://e621.net/post_sets/4", text: "set #4" },
    ]);
  });

  it("links wiki pages and reads tag searches", () => {
    expect(paragraph("[[Tag Group|groups]] [[wolf]] {{wolf male|wolves}} {{fox}}")).toEqual([
      { type: "link", href: "https://e621.net/wiki_pages/show_or_new?title=tag_group", text: "groups" },
      text(" "),
      { type: "link", href: "https://e621.net/wiki_pages/show_or_new?title=wolf", text: "wolf" },
      text(" "),
      { type: "search", tags: ["wolf", "male"], text: "wolves" },
      text(" "),
      { type: "search", tags: ["fox"], text: "fox" },
    ]);
  });
});

describe("blocks", () => {
  it("splits paragraphs at blank lines", () => {
    expect(parseDText("one\r\n\r\ntwo\n\n\nthree")).toEqual([
      { type: "paragraph", children: [text("one")] },
      { type: "paragraph", children: [text("two")] },
      { type: "paragraph", children: [text("three")] },
    ]);
  });

  it("reads a quote with its reply, as comments quote each other", () => {
    expect(parseDText('[quote]"a":/users/1 said:\n[quote]first[/quote]\nsecond\n[/quote]\n\nreply')).toEqual([
      {
        type: "quote",
        children: [
          {
            type: "paragraph",
            children: [{ type: "link", href: "https://e621.net/users/1", text: "a" }, text(" said:")],
          },
          { type: "quote", children: [{ type: "paragraph", children: [text("first")] }] },
          { type: "paragraph", children: [text("second")] },
        ],
      },
      { type: "paragraph", children: [text("reply")] },
    ]);
  });

  it("closes an unclosed quote at the end", () => {
    expect(parseDText("[quote]never closed")).toEqual([
      { type: "quote", children: [{ type: "paragraph", children: [text("never closed")] }] },
    ]);
  });

  it("hides every paragraph of a spoiler that spans several", () => {
    expect(parseDText("before [spoiler]one\n\ntwo[/spoiler] after")).toEqual([
      { type: "paragraph", children: [text("before ")] },
      {
        type: "spoiler",
        children: [
          { type: "paragraph", children: [text("one")] },
          { type: "paragraph", children: [text("two")] },
        ],
      },
      { type: "paragraph", children: [text(" after")] },
    ]);
  });

  it("reads sections with and without a title", () => {
    expect(parseDText("[section,expanded=Notes]a[/section][section]b[/section]")).toEqual([
      { type: "section", title: "Notes", expanded: true, children: [{ type: "paragraph", children: [text("a")] }] },
      { type: "section", title: null, expanded: false, children: [{ type: "paragraph", children: [text("b")] }] },
    ]);
  });

  it("keeps code blocks as written", () => {
    expect(parseDText("[code]\n[b]x[/b]\n  [quote]\n[/code]")).toEqual([{ type: "code", text: "[b]x[/b]\n  [quote]" }]);
  });

  it("reads headings and lists", () => {
    expect(parseDText("h2. Title\nintro\n* one\n** deeper\nafter")).toEqual([
      { type: "heading", level: 2, children: [text("Title")] },
      { type: "paragraph", children: [text("intro")] },
      {
        type: "list",
        items: [
          { depth: 1, children: [text("one")] },
          { depth: 2, children: [text("deeper")] },
        ],
      },
      { type: "paragraph", children: [text("after")] },
    ]);
  });
});
