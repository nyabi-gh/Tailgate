<script lang="ts">
  import { currentToasts, dismiss } from "../lib/toast.svelte";
</script>

<div class="pointer-events-none fixed inset-x-0 bottom-5 z-40 flex flex-col items-center gap-2" aria-live="polite">
  {#each currentToasts() as toast (toast.id)}
    <div class="pointer-events-auto flex items-center gap-3 rounded-md bg-panel py-2 pr-2 pl-3.5 shadow-panel">
      <span>{toast.text}</span>
      {#if toast.action}
        {@const action = toast.action}
        <button
          class="h-7 rounded-sm px-2.5 font-medium text-focus transition-colors hover:bg-hover"
          onclick={() => {
            action.run();
            dismiss(toast.id);
          }}>{action.label}</button
        >
      {/if}
    </div>
  {/each}
</div>
