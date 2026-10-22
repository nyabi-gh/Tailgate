<script lang="ts">
  import { ChevronLeft, ChevronRight } from "@lucide/svelte";
  import { currentLocale, m } from "../lib/i18n/index.svelte";
  import { compare, hasNext, periodEnd, periodLabel, periodStart, step, today, type Scale } from "../lib/popular";
  import { popularPeriod, startPopular } from "../lib/search.svelte";

  const scales: Scale[] = ["day", "week", "month"];

  // Read when the screen opens, so a window left open past midnight catches up on the next visit.
  const now = today();
  const showsToday = $derived(
    compare(periodStart(popularPeriod.day, popularPeriod.scale), now) <= 0 &&
      compare(now, periodEnd(popularPeriod.day, popularPeriod.scale)) <= 0,
  );

  function show(change: Partial<typeof popularPeriod>) {
    Object.assign(popularPeriod, change);
    startPopular();
  }

  const segment = (selected: boolean) => [
    "h-7 rounded-sm px-3 transition-colors",
    selected ? "bg-panel font-medium text-ink shadow-panel" : "text-ink/75 hover:text-ink",
  ];
  const icon =
    "grid size-8 place-items-center rounded-md text-ink/80 transition-colors hover:bg-hover disabled:opacity-35 disabled:hover:bg-transparent";
</script>

<div class="flex min-w-0 items-center gap-3">
  <div class="flex shrink-0 rounded-md bg-hover p-0.5" role="radiogroup" aria-label={m().popular.scale}>
    {#each scales as scale (scale)}
      {@const selected = popularPeriod.scale === scale}
      <button
        class={segment(selected)}
        role="radio"
        aria-checked={selected}
        onclick={() => !selected && show({ scale })}
      >
        {m().popular.scales[scale]}
      </button>
    {/each}
  </div>

  <div class="flex min-w-0 items-center">
    <button
      class={icon}
      aria-label={m().popular.previous}
      title={m().popular.previous}
      onclick={() => show({ day: step(popularPeriod.day, popularPeriod.scale, -1) })}
    >
      <ChevronLeft size={16} strokeWidth={2} />
    </button>
    <span class="min-w-0 truncate px-1.5 text-center font-medium tabular-nums">
      {periodLabel(popularPeriod.day, popularPeriod.scale, currentLocale())}
    </span>
    <button
      class={icon}
      aria-label={m().popular.next}
      title={m().popular.next}
      disabled={!hasNext(popularPeriod.day, popularPeriod.scale, now)}
      onclick={() => show({ day: step(popularPeriod.day, popularPeriod.scale, 1) })}
    >
      <ChevronRight size={16} strokeWidth={2} />
    </button>
  </div>

  <button
    class="h-7 shrink-0 rounded-sm bg-panel px-2.5 text-ink/85 ring-1 ring-seam transition-colors hover:bg-hover disabled:opacity-50"
    disabled={showsToday}
    onclick={() => show({ day: today() })}
  >
    {m().popular.today}
  </button>
</div>
