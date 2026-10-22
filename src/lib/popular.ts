/** Mirrors `Scale` in src-tauri/src/api/endpoints.rs. */
export type Scale = "day" | "week" | "month";

/** A calendar day, month 1–12. Kept apart from `Date` so no time zone shifts it. */
export type Day = { year: number; month: number; day: number };

/** Computed on UTC dates, where every day is 24 hours long. */
const utc = ({ year, month, day }: Day) => new Date(Date.UTC(year, month - 1, day));
const fromUtc = (date: Date): Day => ({
  year: date.getUTCFullYear(),
  month: date.getUTCMonth() + 1,
  day: date.getUTCDate(),
});

export function today(now = new Date()): Day {
  return { year: now.getFullYear(), month: now.getMonth() + 1, day: now.getDate() };
}

export function compare(a: Day, b: Day): number {
  return utc(a).getTime() - utc(b).getTime();
}

/** The first day of the period: e621's weeks start on Monday. */
export function periodStart(day: Day, scale: Scale): Day {
  if (scale === "month") return { ...day, day: 1 };
  if (scale === "day") return day;
  const date = utc(day);
  date.setUTCDate(date.getUTCDate() - ((date.getUTCDay() + 6) % 7));
  return fromUtc(date);
}

export function periodEnd(day: Day, scale: Scale): Day {
  if (scale === "day") return day;
  const date = utc(periodStart(day, scale));
  if (scale === "week") date.setUTCDate(date.getUTCDate() + 6);
  else date.setUTCMonth(date.getUTCMonth() + 1, 0);
  return fromUtc(date);
}

/** The start of the period before or after. */
export function step(day: Day, scale: Scale, by: 1 | -1): Day {
  const date = utc(periodStart(day, scale));
  if (scale === "month") date.setUTCMonth(date.getUTCMonth() + by);
  else date.setUTCDate(date.getUTCDate() + by * (scale === "week" ? 7 : 1));
  return fromUtc(date);
}

/** Periods that have not started yet have no posts. */
export function hasNext(day: Day, scale: Scale, now: Day): boolean {
  return compare(step(day, scale, 1), now) <= 0;
}

export function dateParam({ year, month, day }: Day): string {
  return `${year}-${String(month).padStart(2, "0")}-${String(day).padStart(2, "0")}`;
}

export function periodLabel(day: Day, scale: Scale, locale: string): string {
  if (scale === "day") {
    return new Intl.DateTimeFormat(locale, { dateStyle: "medium", timeZone: "UTC" }).format(utc(day));
  }
  if (scale === "month") {
    return new Intl.DateTimeFormat(locale, { year: "numeric", month: "long", timeZone: "UTC" }).format(utc(day));
  }
  return new Intl.DateTimeFormat(locale, { month: "short", day: "numeric", timeZone: "UTC" }).formatRange(
    utc(periodStart(day, scale)),
    utc(periodEnd(day, scale)),
  );
}
