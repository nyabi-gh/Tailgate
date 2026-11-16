<script lang="ts">
  import { openUrl } from "@tauri-apps/plugin-opener";
  import type { Inline } from "../lib/dtext";
  import DTextInline from "./DTextInline.svelte";
  import DTextSpoiler from "./DTextSpoiler.svelte";

  let { nodes, onsearch }: { nodes: Inline[]; onsearch: (tags: string[]) => void } = $props();

  function open(href: string) {
    openUrl(href).catch((cause) => console.error("could not open a link", cause));
  }

  function onEnter(event: KeyboardEvent, action: () => void) {
    if (event.key !== "Enter") return;
    event.preventDefault();
    action();
  }

  const link = "cursor-pointer text-focus hover:underline";
</script>

{#snippet anchor(text: string, title: string | undefined, action: () => void)}
  <!-- Not an <a href>: a click would take the webview itself to the address. Not a <button>
       either, which cannot wrap across lines inside a sentence. -->
  <span class={link} role="link" tabindex="0" {title} onclick={action} onkeydown={(event) => onEnter(event, action)}>
    {text}
  </span>
{/snippet}

{#each nodes as node, index (index)}
  {#if node.type === "text"}
    {node.text}
  {:else if node.type === "break"}
    <br />
  {:else if node.type === "code"}
    <code class="rounded-sm bg-hover px-1 font-mono text-sm">{node.text}</code>
  {:else if node.type === "link"}
    {@render anchor(node.text, node.href, () => open(node.href))}
  {:else if node.type === "search"}
    {@render anchor(node.text, node.tags.join(" "), () => onsearch(node.tags))}
  {:else if node.type === "spoiler"}
    <DTextSpoiler><DTextInline nodes={node.children} {onsearch} /></DTextSpoiler>
  {:else if node.style === "b"}
    <b class="font-strong"><DTextInline nodes={node.children} {onsearch} /></b>
  {:else if node.style === "i"}
    <i><DTextInline nodes={node.children} {onsearch} /></i>
  {:else if node.style === "u"}
    <u><DTextInline nodes={node.children} {onsearch} /></u>
  {:else if node.style === "s"}
    <s><DTextInline nodes={node.children} {onsearch} /></s>
  {:else if node.style === "sup"}
    <sup><DTextInline nodes={node.children} {onsearch} /></sup>
  {:else}
    <sub><DTextInline nodes={node.children} {onsearch} /></sub>
  {/if}
{/each}
