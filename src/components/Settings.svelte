<script lang="ts">
  import { ChevronDown, CircleAlert } from "@lucide/svelte";
  import { account, signOut, type SignInError } from "../lib/account.svelte";
  import {
    connection,
    connectionDetails,
    importWireGuardConfig,
    mullvadExpiry,
    mullvadLocations,
    mullvadSignOut,
    reconnect,
    setConnectionMode,
    setMullvadLocation,
    type ConnectionDetails,
    type ConnectionMode,
    type LocationChoice,
    type SettingsChangeError,
  } from "../lib/connection.svelte";
  import { settingsChangeMessage } from "../lib/errors";
  import { currentLocale, m, type Language } from "../lib/i18n/index.svelte";
  import { changeGeneralSettings, generalSettings } from "../lib/settings.svelte";
  import ConnectionStatus from "./ConnectionStatus.svelte";
  import MullvadSignInSheet from "./MullvadSignInSheet.svelte";

  let { onsignin }: { onsignin: () => void } = $props();

  const modes: ConnectionMode[] = ["direct", "wireGuard", "mullvad"];
  const languages: Language[] = ["system", "en", "ko"];

  let details = $state<ConnectionDetails | null>(null);
  let expiry = $state<Date | null>(null);
  let locations = $state<LocationChoice[] | "loading" | "failed">("loading");
  let busy = $state(false);
  let error = $state<SettingsChangeError | null>(null);
  let accountError = $state<SignInError | null>(null);
  let generalError = $state<SettingsChangeError | null>(null);
  let mullvadSheet: MullvadSignInSheet;

  const status = $derived(connection());
  const user = $derived(account());
  const signedInToMullvad = $derived(details?.mullvadDevice != null);

  async function refresh() {
    details = await connectionDetails();
  }

  // Every change of the connection may come from new settings (a sign-in, a fresh attempt).
  $effect(() => {
    void status;
    refresh();
  });

  $effect(() => {
    if (details?.mullvadDevice == null) {
      expiry = null;
      return;
    }
    mullvadExpiry().then((date) => (expiry = date));
  });

  $effect(() => {
    if (details?.mode !== "mullvad" || !signedInToMullvad || locations !== "loading") return;
    mullvadLocations().then((result) => (locations = Array.isArray(result) ? result : "failed"));
  });

  /** Runs one change at a time and shows what went wrong, if anything. */
  async function change(action: () => Promise<SettingsChangeError | "cancelled" | null>) {
    busy = true;
    error = null;
    const outcome = await action();
    if (outcome !== null && outcome !== "cancelled") error = outcome;
    await refresh();
    busy = false;
  }

  async function setLanguage(language: Language) {
    generalError = await changeGeneralSettings({ language });
  }

  async function leaveE621() {
    busy = true;
    accountError = await signOut();
    busy = false;
  }

  // Mullvad names countries in English; the system can name them in the app's language. Its codes
  // are ISO 3166 regions, which is what the display names take.
  const regionNames = $derived(new Intl.DisplayNames([currentLocale()], { type: "region" }));
  function countryName(choice: LocationChoice): string {
    const [country = ""] = choice.code.split("-");
    const region = country.toUpperCase();
    try {
      const name = regionNames.of(region);
      return name && name !== region ? name : choice.country;
    } catch {
      // Not a region code the system knows: Mullvad's own name is still right.
      return choice.country;
    }
  }

  const countries = $derived.by(() => {
    if (!Array.isArray(locations)) return [];
    const byCountry = new Map<string, LocationChoice[]>();
    for (const choice of locations) {
      const name = countryName(choice);
      byCountry.set(name, [...(byCountry.get(name) ?? []), choice]);
    }
    const locale = currentLocale();
    return [...byCountry.entries()]
      .sort(([a], [b]) => a.localeCompare(b, locale))
      .map(([name, cities]) => ({ name, cities: cities.sort((a, b) => a.city.localeCompare(b.city, locale)) }));
  });

  const dateFormat = $derived(new Intl.DateTimeFormat(currentLocale(), { dateStyle: "medium" }));

  const group = "flex flex-col divide-y divide-seam rounded-md bg-raised";
  const row = "flex min-h-11 items-center gap-3 px-3.5 py-2";
  const label = "w-28 shrink-0";
  const action =
    "ml-auto h-7 shrink-0 rounded-sm bg-panel px-2.5 text-ink/85 ring-1 ring-seam transition-colors hover:bg-hover disabled:opacity-50";
  const segments = "flex rounded-md bg-hover p-0.5";
  const segment = (selected: boolean) => [
    "h-7 rounded-sm px-3 transition-colors",
    selected ? "bg-panel font-medium text-ink shadow-panel" : "text-ink/75 hover:text-ink",
  ];
</script>

<div class="flex max-w-[600px] flex-col gap-7 px-8 pt-4 pb-10">
  <section class="flex flex-col gap-2">
    <h2 class="px-1 font-strong">{m().settings.general}</h2>
    <div class={group}>
      <div class={row}>
        <span class={label}>{m().settings.language}</span>
        <div class={segments} role="radiogroup" aria-label={m().settings.language}>
          {#each languages as language (language)}
            {@const selected = generalSettings().language === language}
            <button
              class={segment(selected)}
              role="radio"
              aria-checked={selected}
              lang={language === "system" ? undefined : language}
              onclick={() => !selected && setLanguage(language)}
            >
              {m().settings.languages[language]}
            </button>
          {/each}
        </div>
      </div>
    </div>
    {#if generalError}
      <p class="flex items-start gap-2 px-1" role="alert">
        <CircleAlert size={16} strokeWidth={1.75} class="mt-px shrink-0" />{settingsChangeMessage(generalError)}
      </p>
    {/if}
  </section>

  <section class="flex flex-col gap-2">
    <h2 class="px-1 font-strong">{m().settings.account}</h2>
    <div class={group}>
      <div class={row}>
        <span class={label}>{m().settings.e621}</span>
        <span class="truncate text-dim">
          {#if user.state === "signedIn"}
            {user.name}
          {:else if user.state === "failed"}
            {m().account.failure[user.code]}
          {:else}
            {m().settings.notSignedIn}
          {/if}
        </span>
        {#if user.state === "signedIn"}
          <button class={action} disabled={busy} onclick={leaveE621}>{m().account.signOut}</button>
        {:else}
          <button class={action} onclick={onsignin}>
            {user.state === "rejected" ? m().account.signInAgain : m().settings.signIn}
          </button>
        {/if}
      </div>
    </div>
    {#if accountError}
      <p class="flex items-start gap-2 px-1" role="alert">
        <CircleAlert size={16} strokeWidth={1.75} class="mt-px shrink-0" />{accountError === "credentialStore"
          ? m().account.error.credentialStore
          : m().account.error.other}
      </p>
    {/if}
  </section>

  <section class="flex flex-col gap-2">
    <h2 class="px-1 font-strong">{m().settings.connection}</h2>
    <div class={group}>
      <div class={row}>
        <ConnectionStatus />
        {#if status.state === "failed"}
          <span class="min-w-0 truncate text-dim" title={m().connection.failure[status.code]}>
            {m().connection.failure[status.code]}
          </span>
        {/if}
        {#if status.state !== "connecting"}
          <button class={action} onclick={reconnect}>{m().settings.reconnect}</button>
        {/if}
      </div>

      <div class={row}>
        <span class={label}>{m().settings.mode}</span>
        <div class={segments} role="radiogroup" aria-label={m().settings.mode}>
          {#each modes as mode (mode)}
            <button
              class={segment(details?.mode === mode)}
              role="radio"
              aria-checked={details?.mode === mode}
              disabled={busy}
              onclick={() => details?.mode !== mode && change(() => setConnectionMode(mode))}
            >
              {m().settings.modes[mode]}
            </button>
          {/each}
        </div>
      </div>

      {#if details?.mode === "wireGuard"}
        <div class={row}>
          <span class={label}>{m().settings.wireGuard.file}</span>
          <span class="truncate text-dim">{details.wireguard?.fileName ?? m().settings.wireGuard.none}</span>
          <button class={action} disabled={busy} onclick={() => change(importWireGuardConfig)}>
            {details.wireguard ? m().settings.wireGuard.replace : m().settings.wireGuard.choose}
          </button>
        </div>
        {#if details.wireguard}
          <div class={row}>
            <span class={label}>{m().settings.wireGuard.server}</span>
            <span class="truncate text-dim select-text">{details.wireguard.endpoint}</span>
          </div>
        {/if}
      {:else if details?.mode === "mullvad"}
        <div class={row}>
          <span class={label}>{m().settings.mullvad.account}</span>
          {#if signedInToMullvad}
            <span class="truncate text-dim">{details.mullvadDevice}</span>
            {#if expiry}
              <span class="truncate text-faint">{m().settings.mullvad.expires} {dateFormat.format(expiry)}</span>
            {/if}
            <button class={action} disabled={busy} onclick={() => change(mullvadSignOut)}>
              {m().settings.mullvad.signOut}
            </button>
          {:else}
            <span class="truncate text-dim">{m().settings.mullvad.signedOut}</span>
            <button class={action} disabled={busy} onclick={() => mullvadSheet.open()}>
              {m().settings.signIn}
            </button>
          {/if}
        </div>
        {#if signedInToMullvad}
          <div class={row}>
            <span class={label}>{m().settings.mullvad.location}</span>
            {#if locations === "loading"}
              <span class="text-dim">{m().settings.mullvad.loading}</span>
            {:else if locations === "failed"}
              <span class="text-dim">{m().settings.mullvad.locationsFailed}</span>
              <button class={action} onclick={() => (locations = "loading")}>{m().search.retry}</button>
            {:else}
              <label class="relative -ml-3 flex h-8 items-center rounded-md transition-colors hover:bg-hover">
                <span class="sr-only">{m().settings.mullvad.location}</span>
                <select
                  class="h-full appearance-none bg-transparent pr-7 pl-3 outline-none"
                  value={details.location}
                  disabled={busy}
                  onchange={(event) => {
                    const code = event.currentTarget.value;
                    change(() => setMullvadLocation(code));
                  }}
                >
                  {#each countries as country (country.name)}
                    <optgroup label={country.name}>
                      {#each country.cities as choice (choice.code)}
                        <option value={choice.code}>{country.name} · {choice.city}</option>
                      {/each}
                    </optgroup>
                  {/each}
                </select>
                <ChevronDown size={14} strokeWidth={2} class="pointer-events-none absolute right-2.5 text-dim" />
              </label>
            {/if}
          </div>
        {/if}
      {/if}
    </div>

    {#if error}
      <p class="flex items-start gap-2 px-1" role="alert">
        <CircleAlert size={16} strokeWidth={1.75} class="mt-px shrink-0" />{settingsChangeMessage(error)}
      </p>
    {/if}
    {#if details?.mode}
      <p class="px-1 text-dim">
        {details.mode === "direct" ? m().settings.directNote : m().settings.tunnelNote}
      </p>
    {/if}
    {#if details?.mode === "wireGuard"}
      <p class="px-1 text-dim">{m().settings.wireGuardNote}</p>
    {/if}
  </section>
</div>

<MullvadSignInSheet bind:this={mullvadSheet} onsignedin={refresh} />
