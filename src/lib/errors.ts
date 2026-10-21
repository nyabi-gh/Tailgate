import type { SettingsChangeError } from "./connection.svelte";
import { m } from "./i18n/index.svelte";
import type { ApiErrorCode } from "./posts";

/** The words for an API failure; codes without their own message share a general one. */
export function apiErrorMessage(code: ApiErrorCode): string {
  const messages = m().search.error;
  switch (code) {
    case "unauthorized":
    case "network":
    case "rateLimited":
    case "unavailable":
    case "pageLimit":
      return messages[code];
    default:
      return messages.other;
  }
}

export function settingsChangeMessage(failure: SettingsChangeError): string {
  const messages = m().settings.error;
  return failure.code === "invalidConfig" ? messages.invalidConfig(failure.key, failure.missing) : messages[failure.code];
}
