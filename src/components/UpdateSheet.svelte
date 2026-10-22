<script lang="ts">
  import { CircleAlert } from "@lucide/svelte";
  import { m } from "../lib/i18n/index.svelte";
  import { isWindows } from "../lib/platform";
  import {
    appVersion,
    dismissOffer,
    installUpdate,
    offeredUpdate,
    restart,
    updateState,
  } from "../lib/updates.svelte";

  let dialog: HTMLDialogElement;

  const offer = $derived(offeredUpdate());
  const status = $derived(updateState());

  $effect(() => {
    if (offer !== null && !dialog.open) dialog.showModal();
  });

  /** A download in progress keeps going; the sheet comes back from the settings screen. */
  function later() {
    dialog.close();
  }

  const primary =
    "h-8 rounded-md bg-focus px-3.5 font-medium text-panel transition-opacity disabled:opacity-50";
  const secondary = "h-8 rounded-md px-3 text-ink/80 transition-colors hover:bg-hover";
</script>

<dialog
  bind:this={dialog}
  class="m-auto w-[420px] rounded-lg bg-panel p-0 text-sm text-ink shadow-panel backdrop:bg-window/60"
  onclose={dismissOffer}
>
  {#if offer}
    <div class="flex flex-col gap-4 p-5">
      <div class="flex flex-col gap-1.5">
        <h2 class="text-base font-strong">{m().update.title(offer.version)}</h2>
        <p class="text-dim">{m().update.current(appVersion())}</p>
      </div>

      {#if offer.notes}
        <p class="max-h-48 overflow-y-auto rounded-md bg-raised px-3 py-2.5 whitespace-pre-wrap select-text">
          {offer.notes}
        </p>
      {/if}

      {#if status.phase === "downloading"}
        <div class="flex flex-col gap-1.5">
          <p class="text-dim">{m().update.downloading(status.percent)}</p>
          {#if status.percent !== null}
            <div class="h-1 overflow-hidden rounded-full bg-hover">
              <div class="h-full rounded-full bg-focus transition-[width]" style:width="{status.percent}%"></div>
            </div>
          {/if}
        </div>
      {:else if status.phase === "installed"}
        <p>{m().update.installed}</p>
      {:else if status.phase === "installFailed"}
        <p class="flex items-start gap-2" role="alert">
          <CircleAlert size={16} strokeWidth={1.75} class="mt-px shrink-0" />{m().update.failure[status.code]}
        </p>
      {:else if isWindows}
        <p class="text-dim">{m().update.windowsNote}</p>
      {/if}

      <div class="flex justify-end gap-2">
        <button type="button" class={secondary} onclick={later}>{m().update.later}</button>
        {#if status.phase === "installed"}
          <button type="button" class={primary} onclick={restart}>{m().update.restart}</button>
        {:else}
          <button type="button" class={primary} disabled={status.phase === "downloading"} onclick={installUpdate}>
            {status.phase === "installFailed" ? m().update.retry : m().update.install}
          </button>
        {/if}
      </div>
    </div>
  {/if}
</dialog>
