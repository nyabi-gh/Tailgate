<script lang="ts">
  import { Check, CircleAlert, LoaderCircle } from "@lucide/svelte";
  import { apiErrorMessage } from "../lib/errors";
  import { m } from "../lib/i18n/index.svelte";
  import { apiErrorCode, type ApiErrorCode } from "../lib/posts";
  import { changeSet, setChoices, type SetChoice } from "../lib/sets.svelte";

  let { post }: { post: number } = $props();

  let choices = $state<SetChoice[] | null>(null);
  let failed = $state<ApiErrorCode | null>(null);
  /** The sets with a change on its way, so a second click waits for the first. */
  let changing = $state<Set<number>>(new Set());
  let changeFailed = $state<string | null>(null);

  // Read for each post: what is in which set may have changed on the site.
  $effect(() => {
    const asked = post;
    choices = null;
    failed = null;
    changeFailed = null;
    setChoices(asked).then(
      (found) => {
        if (asked === post) choices = found;
      },
      (cause) => {
        if (asked === post) failed = apiErrorCode(cause);
      },
    );
  });

  /** Shown at once, and undone if e621 refuses. */
  async function toggle(choice: SetChoice) {
    const add = !choice.contains;
    const set = (contains: boolean) =>
      (choices = choices?.map((other) => (other.id === choice.id ? { ...other, contains } : other)) ?? null);
    changing = new Set([...changing, choice.id]);
    changeFailed = null;
    set(add);
    try {
      await changeSet(choice.id, post, add);
    } catch (cause) {
      set(!add);
      changeFailed = `${m().sets.menu.failed} ${apiErrorMessage(apiErrorCode(cause))}`;
    } finally {
      changing = new Set([...changing].filter((id) => id !== choice.id));
    }
  }
</script>

<div class="flex flex-col gap-1 rounded-md bg-raised p-1.5">
  {#if failed !== null}
    <p class="flex items-start gap-2 p-1.5" role="alert">
      <CircleAlert size={14} strokeWidth={1.75} class="mt-0.5 shrink-0" />{apiErrorMessage(failed)}
    </p>
  {:else if choices === null}
    <p class="flex items-center gap-2 p-1.5 text-dim">
      <LoaderCircle size={14} strokeWidth={2} class="animate-spin" />{m().sets.menu.loading}
    </p>
  {:else if choices.length === 0}
    <p class="p-1.5 text-dim">{m().sets.menu.none}</p>
  {:else}
    <ul class="flex max-h-64 flex-col overflow-y-auto">
      {#each choices as choice (choice.id)}
        <li>
          <button
            class="flex w-full items-center gap-2 rounded-sm px-1.5 py-1 text-left transition-colors hover:bg-hover disabled:opacity-60"
            role="menuitemcheckbox"
            aria-checked={choice.contains}
            disabled={changing.has(choice.id)}
            onclick={() => toggle(choice)}
          >
            <span
              class={[
                "grid size-4 shrink-0 place-items-center rounded-sm ring-1",
                choice.contains ? "bg-focus text-panel ring-focus" : "ring-seam",
              ]}
            >
              {#if choice.contains}<Check size={12} strokeWidth={3} />{/if}
            </span>
            <span class="truncate">{choice.name}</span>
          </button>
        </li>
      {/each}
    </ul>
  {/if}
  {#if changeFailed}
    <p class="flex items-start gap-2 p-1.5" role="alert">
      <CircleAlert size={14} strokeWidth={1.75} class="mt-0.5 shrink-0" />{changeFailed}
    </p>
  {/if}
</div>
