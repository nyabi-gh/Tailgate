import { describe, expect, it } from "vitest";
import { dateParam, hasNext, periodEnd, periodLabel, periodStart, step, today, type Day } from "./popular";

const day = (year: number, month: number, date: number): Day => ({ year, month, day: date });

describe("popular periods", () => {
  it("starts weeks on Monday, as e621 does", () => {
    // 2026-10-22 is a Thursday.
    expect(periodStart(day(2026, 10, 22), "week")).toEqual(day(2026, 10, 19));
    expect(periodEnd(day(2026, 10, 22), "week")).toEqual(day(2026, 10, 25));
    // A Sunday belongs to the week that began six days before.
    expect(periodStart(day(2026, 10, 25), "week")).toEqual(day(2026, 10, 19));
  });

  it("covers whole months, leap days included", () => {
    expect(periodStart(day(2028, 2, 17), "month")).toEqual(day(2028, 2, 1));
    expect(periodEnd(day(2028, 2, 17), "month")).toEqual(day(2028, 2, 29));
    expect(periodEnd(day(2026, 12, 3), "month")).toEqual(day(2026, 12, 31));
  });

  it("steps across month and year ends", () => {
    expect(step(day(2026, 12, 31), "day", 1)).toEqual(day(2027, 1, 1));
    expect(step(day(2026, 3, 1), "day", -1)).toEqual(day(2026, 2, 28));
    expect(step(day(2026, 12, 30), "week", 1)).toEqual(day(2027, 1, 4));
    // From the 31st, a month step lands on the next month's first day, never skipping one.
    expect(step(day(2026, 1, 31), "month", 1)).toEqual(day(2026, 2, 1));
    expect(step(day(2026, 1, 15), "month", -1)).toEqual(day(2025, 12, 1));
  });

  it("offers no step into a period that has not started", () => {
    const now = day(2026, 10, 22);
    expect(hasNext(now, "day", now)).toBe(false);
    expect(hasNext(day(2026, 10, 21), "day", now)).toBe(true);
    expect(hasNext(day(2026, 10, 15), "week", now)).toBe(true);
    expect(hasNext(now, "week", now)).toBe(false);
    expect(hasNext(day(2026, 9, 30), "month", now)).toBe(true);
    expect(hasNext(now, "month", now)).toBe(false);
  });

  it("names the day by the local calendar", () => {
    // 23:30 local time is still that day, whatever the UTC date is.
    expect(today(new Date(2026, 9, 22, 23, 30))).toEqual(day(2026, 10, 22));
    expect(dateParam(day(2026, 1, 5))).toBe("2026-01-05");
  });

  it("labels each scale", () => {
    expect(periodLabel(day(2026, 10, 22), "day", "en")).toBe("Oct 22, 2026");
    // ICU puts thin spaces around the range dash.
    expect(periodLabel(day(2026, 10, 22), "week", "en")).toMatch(/^Oct 19\s–\s25$/u);
    expect(periodLabel(day(2026, 10, 22), "month", "en")).toBe("October 2026");
    expect(periodLabel(day(2026, 10, 22), "month", "ko")).toBe("2026년 10월");
  });
});
