<script lang="ts">
  import { CircleAlert, ExternalLink, Eye, EyeOff } from "@lucide/svelte";
  import { openUrl } from "@tauri-apps/plugin-opener";
  import { account, signIn, signOut, type SignInError } from "../lib/account.svelte";
  import { m } from "../lib/i18n/index.svelte";

  const API_KEYS_URL = "https://e621.net/api_keys";

  let dialog: HTMLDialogElement;
  let username = $state("");
  let apiKey = $state("");
  let showKey = $state(false);
  let busy = $state(false);
  let error = $state<SignInError | null>(null);

  const status = $derived(account());

  export function open() {
    error = null;
    showKey = false;
    if (status.state === "rejected") username = status.name;
    dialog.showModal();
  }

  function close() {
    dialog.close();
  }

  function message(code: SignInError): string {
    const messages = m().account.error;
    switch (code) {
      case "unauthorized":
      case "network":
      case "rateLimited":
      case "unavailable":
      case "credentialStore":
        return messages[code];
      default:
        return messages.other;
    }
  }

  async function submit(event: SubmitEvent) {
    event.preventDefault();
    busy = true;
    error = await signIn(username, apiKey);
    // The key stays only in Rust once it has been sent.
    apiKey = "";
    busy = false;
    if (error === null) close();
  }

  async function leave() {
    busy = true;
    error = await signOut();
    busy = false;
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
  {#if status.state === "signedIn"}
    <div class="flex flex-col gap-4 p-5">
      <h2 class="text-base font-strong">{m().account.title}</h2>
      <p class="text-dim">{m().account.signedInAs(status.name)}</p>
      {#if error}
        <p class="flex items-start gap-2 text-ink" role="alert">
          <CircleAlert size={16} strokeWidth={1.75} class="mt-px shrink-0" />{message(error)}
        </p>
      {/if}
      <div class="flex justify-end gap-2">
        <button class={secondary} onclick={close}>{m().account.close}</button>
        <button class={primary} disabled={busy} onclick={leave}>{m().account.signOut}</button>
      </div>
    </div>
  {:else}
    <form class="flex flex-col gap-4 p-5" onsubmit={submit}>
      <div class="flex flex-col gap-1.5">
        <h2 class="text-base font-strong">{m().account.title}</h2>
        {#if status.state === "rejected"}
          <p class="flex items-start gap-2 text-ink">
            <CircleAlert size={16} strokeWidth={1.75} class="mt-px shrink-0" />{m().account.rejected(status.name)}
          </p>
        {:else if status.state === "failed"}
          <p class="flex items-start gap-2 text-ink">
            <CircleAlert size={16} strokeWidth={1.75} class="mt-px shrink-0" />{m().account.failure[status.code]}
          </p>
        {/if}
        <p class="text-dim">{m().account.body}</p>
      </div>

      <label class="flex flex-col gap-1.5">
        <span class="font-medium">{m().account.username}</span>
        <input
          bind:value={username}
          class={field}
          type="text"
          autocomplete="username"
          spellcheck="false"
          required
          disabled={busy}
        />
      </label>

      <div class="flex flex-col gap-1.5">
        <label for="api-key" class="font-medium">{m().account.apiKey}</label>
        <div class={[field, "flex items-center gap-1 pr-1"]}>
          <input
            id="api-key"
            bind:value={apiKey}
            class="min-w-0 flex-1 bg-transparent outline-none"
            type={showKey ? "text" : "password"}
            autocomplete="off"
            spellcheck="false"
            required
            disabled={busy}
          />
          <button
            type="button"
            class="grid size-7 place-items-center rounded-sm text-dim transition-colors hover:bg-hover hover:text-ink"
            aria-label={showKey ? m().account.hideKey : m().account.showKey}
            title={showKey ? m().account.hideKey : m().account.showKey}
            onclick={() => (showKey = !showKey)}
          >
            {#if showKey}<EyeOff size={15} strokeWidth={1.75} />{:else}<Eye size={15} strokeWidth={1.75} />{/if}
          </button>
        </div>
        <button
          type="button"
          class="flex w-fit items-center gap-1 text-focus hover:underline"
          onclick={() => openUrl(API_KEYS_URL).catch((cause) => console.error("could not open the API key page", cause))}
        >
          {m().account.getKey}<ExternalLink size={13} strokeWidth={1.75} />
        </button>
      </div>

      {#if error}
        <p class="flex items-start gap-2 text-ink" role="alert">
          <CircleAlert size={16} strokeWidth={1.75} class="mt-px shrink-0" />{message(error)}
        </p>
      {/if}

      <div class="flex justify-end gap-2">
        <button type="button" class={secondary} onclick={close}>{m().account.cancel}</button>
        <button type="submit" class={primary} disabled={busy || !username.trim() || !apiKey.trim()}>
          {busy ? m().account.submitting : m().account.submit}
        </button>
      </div>
    </form>
  {/if}
</dialog>
