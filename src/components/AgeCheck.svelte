<script lang="ts">
  import icon from "../../src-tauri/icons/128x128@2x.png";
  import { m } from "../lib/i18n/index.svelte";
  import { changeGeneralSettings, quit } from "../lib/settings.svelte";
  import { notify } from "../lib/toast.svelte";

  async function confirm() {
    // The toast shows once the app has replaced this screen.
    if ((await changeGeneralSettings({ adult: true })) !== null) notify(m().ageCheck.notSaved);
  }

  const primary = "h-8 rounded-md bg-focus px-3.5 font-medium text-panel transition-opacity";
  const secondary = "h-8 rounded-md px-3 text-ink/80 transition-colors hover:bg-hover";
</script>

<div class="flex h-full items-center justify-center px-8" data-tauri-drag-region>
  <div class="flex max-w-[400px] flex-col items-center gap-5 text-center">
    <img src={icon} alt="" class="size-20" draggable="false" />
    <div class="flex flex-col gap-2">
      <h1 class="text-lg font-strong">{m().ageCheck.title}</h1>
      <p class="text-dim">{m().ageCheck.body}</p>
    </div>
    <div class="flex gap-2">
      <button class={secondary} onclick={quit}>{m().ageCheck.quit}</button>
      <button class={primary} onclick={confirm}>{m().ageCheck.confirm}</button>
    </div>
  </div>
</div>
