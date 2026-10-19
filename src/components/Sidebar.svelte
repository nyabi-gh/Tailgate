<script lang="ts">
  import { BookOpen, CircleUserRound, Flame, Heart, Layers, Search, Settings } from "@lucide/svelte";
  import { m } from "../lib/i18n/index.svelte";
  import { browseViews, type BrowseView, type View } from "../lib/views";

  let { current, onselect }: { current: View; onselect: (view: View) => void } = $props();

  const icons = { search: Search, favorites: Heart, popular: Flame, pools: BookOpen, sets: Layers } satisfies Record<
    BrowseView,
    unknown
  >;

  const item = "flex h-8 w-full items-center gap-2.5 rounded-md px-2.5 text-left transition-colors hover:bg-hover";
</script>

<nav class="flex w-sidebar shrink-0 flex-col px-2.5 pb-2.5">
  <div class="h-titlebar shrink-0" data-tauri-drag-region></div>

  <ul class="flex flex-col gap-px">
    {#each browseViews as view (view)}
      {@const Icon = icons[view]}
      <li>
        <button
          class={[item, current === view ? "bg-selected font-medium text-ink" : "text-ink/75"]}
          aria-current={current === view ? "page" : undefined}
          onclick={() => onselect(view)}
        >
          <Icon size={16} strokeWidth={1.75} class={current === view ? "text-focus" : "text-dim"} />
          {m().nav[view]}
        </button>
      </li>
    {/each}
  </ul>

  <div class="mt-auto flex flex-col gap-px">
    <button class={[item, "text-dim"]} onclick={() => onselect("settings")}>
      <span class="mx-[5px] size-1.5 rounded-full bg-faint"></span>
      {m().connection.direct}
    </button>

    <div class="flex items-center gap-1">
      <button class={[item, "flex-1 text-ink/75"]} onclick={() => onselect("settings")}>
        <CircleUserRound size={16} strokeWidth={1.75} class="text-dim" />
        {m().account.signIn}
      </button>
      <button
        class={[
          "grid size-8 shrink-0 place-items-center rounded-md transition-colors hover:bg-hover",
          current === "settings" ? "bg-selected text-focus" : "text-dim",
        ]}
        aria-label={m().nav.settings}
        title={m().nav.settings}
        onclick={() => onselect("settings")}
      >
        <Settings size={16} strokeWidth={1.75} />
      </button>
    </div>
  </div>
</nav>
