<script lang="ts">
  import { CircleAlert, Eye, EyeOff } from "@lucide/svelte";
  import { mullvadSignIn, type SettingsChangeError } from "../lib/connection.svelte";
  import { settingsChangeMessage } from "../lib/errors";
  import { m } from "../lib/i18n/index.svelte";

  let { onsignedin }: { onsignedin: () => void } = $props();

  let dialog: HTMLDialogElement;
  let accountNumber = $state("");
  let show = $state(false);
  let busy = $state(false);
  let error = $state<SettingsChangeError | null>(null);

  export function open() {
    error = null;
    show = false;
    dialog.showModal();
  }

  async function submit(event: SubmitEvent) {
    event.preventDefault();
    busy = true;
    error = await mullvadSignIn(accountNumber);
    // The number stays only in Rust once it has been sent.
    accountNumber = "";
    busy = false;
    if (error === null) {
      dialog.close();
      onsignedin();
    }
  }

  const field =
    "h-9 w-full rounded-md bg-raised px-3 text-ink ring-1 ring-transparent outline-none select-text transition-[box-shadow,background-color] focus-within:bg-panel focus-within:shadow-[0_0_0_4px] focus-within:shadow-focus/15 focus-within:ring-focus/60";
  const primary =
    "h-8 rounded-md bg-focus px-3.5 font-medium text-panel transition-opacity disabled:opacity-50";
  const secondary = "h-8 rounded-md px-3 text-ink/80 transition-colors hover:bg-hover";
</script>

<dialog
  bind:this={dialog}
  class="m-auto w-[380px] rounded-lg bg-panel p-0 text-sm text-ink shadow-panel backdrop:bg-window/60"
  onclose={() => (error = null)}
>
  <form class="flex flex-col gap-4 p-5" onsubmit={submit}>
    <div class="flex flex-col gap-1.5">
      <h2 class="text-base font-strong">{m().settings.mullvadSheet.title}</h2>
      <p class="text-dim">{m().settings.mullvadSheet.body}</p>
    </div>

    <div class="flex flex-col gap-1.5">
      <label for="mullvad-account" class="font-medium">{m().settings.mullvadSheet.number}</label>
      <div class={[field, "flex items-center gap-1 pr-1"]}>
        <input
          id="mullvad-account"
          bind:value={accountNumber}
          class="min-w-0 flex-1 bg-transparent tracking-wide outline-none"
          type={show ? "text" : "password"}
          inputmode="numeric"
          autocomplete="off"
          spellcheck="false"
          required
          disabled={busy}
        />
        <button
          type="button"
          class="grid size-7 place-items-center rounded-sm text-dim transition-colors hover:bg-hover hover:text-ink"
          aria-label={show ? m().settings.mullvadSheet.hide : m().settings.mullvadSheet.show}
          title={show ? m().settings.mullvadSheet.hide : m().settings.mullvadSheet.show}
          onclick={() => (show = !show)}
        >
          {#if show}<EyeOff size={15} strokeWidth={1.75} />{:else}<Eye size={15} strokeWidth={1.75} />{/if}
        </button>
      </div>
    </div>

    {#if error}
      <p class="flex items-start gap-2 text-ink" role="alert">
        <CircleAlert size={16} strokeWidth={1.75} class="mt-px shrink-0" />{settingsChangeMessage(error)}
      </p>
    {/if}

    <div class="flex justify-end gap-2">
      <button type="button" class={secondary} onclick={() => dialog.close()}>{m().settings.mullvadSheet.cancel}</button>
      <button type="submit" class={primary} disabled={busy || !accountNumber.trim()}>
        {busy ? m().settings.mullvadSheet.submitting : m().settings.mullvadSheet.submit}
      </button>
    </div>
  </form>
</dialog>
