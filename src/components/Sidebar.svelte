<script lang="ts">
  import { BookOpen, CircleAlert, CircleUserRound, Flame, Heart, Layers, Search, Settings } from "@lucide/svelte";
  import { account } from "../lib/account.svelte";
  import { connection } from "../lib/connection.svelte";
  import { m } from "../lib/i18n/index.svelte";
  import { browseViews, type BrowseView, type View } from "../lib/views";
  import ConnectionStatus from "./ConnectionStatus.svelte";

  let {
    current,
    onselect,
    onaccount,
  }: { current: View; onselect: (view: View) => void; onaccount: () => void } = $props();

  const icons = { search: Search, favorites: Heart, popular: Flame, pools: BookOpen, sets: Layers } satisfies Record<
    BrowseView,
    unknown
  >;

  const item = "flex h-8 w-full items-center gap-2.5 rounded-md px-2.5 text-left transition-colors hover:bg-hover";

  const status = $derived(connection());
  const user = $derived(account());
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
    <button
      class={[item, status.state === "failed" ? "text-ink" : "text-dim"]}
      title={status.state === "failed" ? m().connection.failure[status.code] : undefined}
      onclick={() => onselect("settings")}
    >
      <ConnectionStatus />
    </button>

    <div class="flex items-center gap-1">
      <button
        class={[item, "min-w-0 flex-1", user.state === "rejected" ? "text-ink" : "text-ink/75"]}
        title={user.state === "failed" ? m().account.failure[user.code] : undefined}
        onclick={onaccount}
      >
        {#if user.state === "rejected" || user.state === "failed"}
          <CircleAlert size={16} strokeWidth={1.75} class="shrink-0" />
        {:else}
          <CircleUserRound size={16} strokeWidth={1.75} class="shrink-0 text-dim" />
        {/if}
        <span class="truncate">
          {#if user.state === "signedIn"}
            {user.name}
          {:else if user.state === "rejected"}
            {m().account.signInAgain}
          {:else}
            {m().account.signIn}
          {/if}
        </span>
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
