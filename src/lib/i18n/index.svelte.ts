import en, { type Messages } from "./en";
import ko from "./ko";

export type Locale = "en" | "ko";

const catalogs: Record<Locale, Messages> = { en, ko };

function systemLocale(): Locale {
  return navigator.language.toLowerCase().startsWith("ko") ? "ko" : "en";
}

let locale = $state<Locale>("en");

export function currentLocale(): Locale {
  return locale;
}

export function setLocale(next: Locale) {
  locale = next;
  document.documentElement.lang = next;
}

/** Read inside markup as `m().nav.search` so the text re-renders when the locale changes. */
export function m(): Messages {
  return catalogs[locale];
}

setLocale(systemLocale());
