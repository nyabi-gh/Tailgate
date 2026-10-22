<script lang="ts">
  import { ChevronDown, CircleAlert } from "@lucide/svelte";
  import { openUrl } from "@tauri-apps/plugin-opener";
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
  import {
    chooseDownloadFolder,
    downloadDetails,
    exampleName,
    isNameRuleError,
    openDownloadFolder,
    setDownloadName,
    type DownloadDetails,
    type NameRuleError,
  } from "../lib/downloadSettings";
  import { settingsChangeMessage } from "../lib/errors";
  import {
    CACHE_LIMITS_MB,
    cacheUsage,
    clearCache,
    formatBytes,
    megabytes,
    setCacheLimit,
    type CacheUsage,
  } from "../lib/storage";
  import { currentLocale, m, type Language } from "../lib/i18n/index.svelte";
  import { CELL_WIDTH } from "../lib/layout";
  import {
    cellWidth,
    changeGeneralSettings,
    clearRecentSearches,
    generalSettings,
    recentSearches,
    setCellWidth,
  } from "../lib/settings.svelte";
  import {
    appVersion,
    checkForUpdate,
    checksAutomatically,
    openNotices,
    restart,
    setChecksAutomatically,
    showUpdate,
    updateState,
  } from "../lib/updates.svelte";
  import ConnectionStatus from "./ConnectionStatus.svelte";
  import MullvadSignInSheet from "./MullvadSignInSheet.svelte";

  let { onsignin }: { onsignin: () => void } = $props();

  const SOURCE_URL = "https://github.com/nyabi-gh/Tailgate";
  const modes: ConnectionMode[] = ["direct", "wireGuard", "mullvad"];
  const languages: Language[] = ["system", "en", "ko"];

  let details = $state<ConnectionDetails | null>(null);
  let expiry = $state<Date | null>(null);
  let locations = $state<LocationChoice[] | "loading" | "failed">("loading");
  let busy = $state(false);
  let error = $state<SettingsChangeError | null>(null);
  let accountError = $state<SignInError | null>(null);
  let generalError = $state<SettingsChangeError | null>(null);
  let downloads = $state<DownloadDetails | null>(null);
  let nameDraft = $state("");
  let nameCheck = $state<{ example: string } | { error: NameRuleError } | null>(null);
  let downloadsError = $state<string | null>(null);
  let cache = $state<CacheUsage | null>(null);
  let cacheBusy = $state(false);
  let cacheError = $state<string | null>(null);
  let aboutError = $state<string | null>(null);
  let mullvadSheet: MullvadSignInSheet;

  const status = $derived(connection());
  const update = $derived(updateState());
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

  cacheUsage().then((usage) => (cache = usage));

  /** A limit kept from elsewhere than this list still shows as chosen. */
  const cacheLimits = $derived(
    cache === null || CACHE_LIMITS_MB.includes(megabytes(cache.limit))
      ? CACHE_LIMITS_MB
      : [...CACHE_LIMITS_MB, megabytes(cache.limit)].sort((a, b) => a - b),
  );

  async function clearImages() {
    cacheBusy = true;
    cacheError = null;
    const outcome = await clearCache();
    if (outcome === "failed") {
      cacheError = m().settings.storage.clearFailed;
      cache = await cacheUsage();
    } else {
      cache = outcome;
    }
    cacheBusy = false;
  }

  async function changeCacheLimit(limitMb: number) {
    cacheBusy = true;
    cacheError = null;
    const outcome = await setCacheLimit(limitMb);
    if ("code" in outcome) cacheError = settingsChangeMessage(outcome);
    else cache = outcome;
    cacheBusy = false;
  }

  async function refreshDownloads() {
    downloads = await downloadDetails();
    nameDraft = downloads.name;
    nameCheck = downloads.example === null ? null : { example: downloads.example };
  }
  refreshDownloads();

  async function chooseFolder() {
    downloadsError = null;
    const outcome = await chooseDownloadFolder();
    if (outcome !== null && outcome !== "cancelled") downloadsError = settingsChangeMessage(outcome);
    await refreshDownloads();
  }

  async function openFolder() {
    downloadsError = (await openDownloadFolder()) ? null : m().settings.downloads.openFailed;
  }

  async function checkName() {
    const draft = nameDraft;
    const check = await exampleName(draft);
    // Answers to earlier keystrokes may come back after later ones.
    if (draft === nameDraft) nameCheck = check;
  }

  /** On Enter or leaving the field; a rule that cannot be used stays unsaved with its reason. */
  async function saveName() {
    if (downloads === null || nameDraft === downloads.name) return;
    downloadsError = null;
    const outcome = await setDownloadName(nameDraft);
    if (outcome === null) downloads = { ...downloads, name: nameDraft };
    else if (isNameRuleError(outcome)) nameCheck = { error: outcome };
    else downloadsError = settingsChangeMessage(outcome);
  }

  function ruleMessage(error: NameRuleError): string {
    const messages = m().settings.downloads.rule;
    return error.code === "unknownToken" ? messages.unknownToken(error.token) : messages[error.code];
  }

  async function showLicenses() {
    aboutError = (await openNotices()) ? null : m().settings.about.licensesFailed;
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
  const button =
    "h-7 shrink-0 rounded-sm bg-panel px-2.5 text-ink/85 ring-1 ring-seam transition-colors hover:bg-hover disabled:opacity-50";
  const action = `ml-auto ${button}`;
  const field =
    "h-8 w-full rounded-sm bg-panel px-2.5 text-ink ring-1 ring-seam outline-none select-text transition-[box-shadow] focus:shadow-[0_0_0_3px] focus:shadow-focus/15 focus:ring-focus/60";
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

  <section class="flex flex-col gap-2">
    <h2 class="px-1 font-strong">{m().settings.browse}</h2>
    <div class={group}>
      <div class={row}>
        <label class={label} for="thumbnail-size">{m().settings.thumbnailSize}</label>
        <input
          id="thumbnail-size"
          type="range"
          class="w-48 accent-focus"
          min={CELL_WIDTH.min}
          max={CELL_WIDTH.max}
          step="10"
          value={cellWidth()}
          oninput={(event) => setCellWidth(Number(event.currentTarget.value))}
        />
      </div>
      <div class={row}>
        <span class={label}>{m().settings.recentSearches}</span>
        <span class="truncate text-dim">{m().settings.recentCount(recentSearches().length)}</span>
        <button class={action} disabled={recentSearches().length === 0} onclick={clearRecentSearches}>
          {m().settings.clearRecent}
        </button>
      </div>
    </div>
  </section>

  <section class="flex flex-col gap-2">
    <h2 class="px-1 font-strong">{m().settings.downloads.title}</h2>
    <div class={group}>
      <div class={row}>
        <span class={label}>{m().settings.downloads.folder}</span>
        <span class="min-w-0 truncate text-dim select-text" title={downloads?.folder}>{downloads?.folder}</span>
        <div class="ml-auto flex shrink-0 gap-2">
          <button class={button} onclick={openFolder}>{m().settings.downloads.open}</button>
          <button class={button} onclick={chooseFolder}>{m().settings.downloads.choose}</button>
        </div>
      </div>
      <div class={[row, "items-start py-3"]}>
        <label class={[label, "pt-1.5"]} for="file-name">{m().settings.downloads.fileName}</label>
        <div class="flex min-w-0 flex-1 flex-col gap-1.5">
          <input
            id="file-name"
            class={field}
            type="text"
            spellcheck="false"
            autocomplete="off"
            bind:value={nameDraft}
            oninput={checkName}
            onchange={saveName}
            onkeydown={(event) => event.key === "Enter" && event.currentTarget.blur()}
            aria-describedby="file-name-check"
          />
          {#if nameCheck !== null && "error" in nameCheck}
            <p id="file-name-check" class="flex items-start gap-1.5 text-ink" role="alert">
              <CircleAlert size={14} strokeWidth={1.75} class="mt-0.5 shrink-0" />{ruleMessage(nameCheck.error)}
            </p>
          {:else if nameCheck !== null}
            <p id="file-name-check" class="truncate text-dim select-text">
              {m().settings.downloads.example(nameCheck.example)}
            </p>
          {/if}
          <p class="text-xs text-faint">{m().settings.downloads.tokens}</p>
        </div>
      </div>
    </div>
    {#if downloadsError}
      <p class="flex items-start gap-2 px-1" role="alert">
        <CircleAlert size={16} strokeWidth={1.75} class="mt-px shrink-0" />{downloadsError}
      </p>
    {/if}
  </section>

  <section class="flex flex-col gap-2">
    <h2 class="px-1 font-strong">{m().settings.storage.title}</h2>
    <div class={group}>
      <div class={row}>
        <span class={label}>{m().settings.storage.images}</span>
        {#if cache}
          <span class="truncate text-dim">{m().settings.storage.used(formatBytes(cache.used, currentLocale()))}</span>
        {/if}
        <button class={action} disabled={cacheBusy || cache?.used === 0} onclick={clearImages}>
          {m().settings.storage.clear}
        </button>
      </div>
      <div class={row}>
        <span class={label}>{m().settings.storage.limit}</span>
        {#if cache}
          <label class="relative -ml-3 flex h-8 items-center rounded-md transition-colors hover:bg-hover">
            <span class="sr-only">{m().settings.storage.limit}</span>
            <select
              class="h-full appearance-none bg-transparent pr-7 pl-3 outline-none"
              value={megabytes(cache.limit)}
              disabled={cacheBusy}
              onchange={(event) => changeCacheLimit(Number(event.currentTarget.value))}
            >
              {#each cacheLimits as limit (limit)}
                <option value={limit}>{formatBytes(limit * 1024 * 1024, currentLocale())}</option>
              {/each}
            </select>
            <ChevronDown size={14} strokeWidth={2} class="pointer-events-none absolute right-2.5 text-dim" />
          </label>
        {/if}
      </div>
    </div>
    {#if cacheError}
      <p class="flex items-start gap-2 px-1" role="alert">
        <CircleAlert size={16} strokeWidth={1.75} class="mt-px shrink-0" />{cacheError}
      </p>
    {/if}
    <p class="px-1 text-dim">{m().settings.storage.note}</p>
  </section>

  <section class="flex flex-col gap-2">
    <h2 class="px-1 font-strong">{m().settings.about.title}</h2>
    <div class={group}>
      <div class={row}>
        <span class={label}>{m().settings.about.version}</span>
        <span class="shrink-0 select-text">{appVersion()}</span>
        <span class="min-w-0 truncate text-dim">
          {#if update.phase === "checking"}
            {m().settings.about.checking}
          {:else if update.phase === "upToDate"}
            {m().settings.about.upToDate}
          {:else if update.phase === "checkFailed"}
            {m().settings.about.checkFailed}
          {:else if "update" in update}
            {m().settings.about.available(update.update.version)}
          {/if}
        </span>
        {#if update.phase === "installed"}
          <button class={action} onclick={restart}>{m().update.restart}</button>
        {:else if "update" in update}
          <button class={action} onclick={showUpdate}>{m().settings.about.showUpdate}</button>
        {:else}
          <button class={action} disabled={update.phase === "checking"} onclick={checkForUpdate}>
            {m().settings.about.check}
          </button>
        {/if}
      </div>
      <label class={[row, "cursor-default"]}>
        <span class={label}>{m().settings.about.automatic}</span>
        <input
          type="checkbox"
          class="ml-auto size-4 accent-focus"
          checked={checksAutomatically()}
          onchange={(event) => setChecksAutomatically(event.currentTarget.checked)}
        />
      </label>
      <div class={row}>
        <span class={label}>{m().settings.about.licenses}</span>
        <button class={action} onclick={showLicenses}>{m().settings.about.open}</button>
      </div>
      <div class={row}>
        <span class={label}>{m().settings.about.source}</span>
        <span class="min-w-0 truncate text-dim select-text">github.com/nyabi-gh/Tailgate</span>
        <button
          class={action}
          onclick={() => openUrl(SOURCE_URL).catch((cause) => console.error("could not open the source page", cause))}
        >
          {m().settings.about.open}
        </button>
      </div>
    </div>
    {#if aboutError}
      <p class="flex items-start gap-2 px-1" role="alert">
        <CircleAlert size={16} strokeWidth={1.75} class="mt-px shrink-0" />{aboutError}
      </p>
    {/if}
    <p class="px-1 text-dim">{m().settings.about.note}</p>
  </section>
</div>

<MullvadSignInSheet bind:this={mullvadSheet} onsignedin={refresh} />
