import { invoke } from "@tauri-apps/api/core";
import { isChangeError, type SettingsChangeError } from "./connection.svelte";

/** Mirrors `CacheUsage` in src-tauri/src/media/disk.rs, in bytes. */
export type CacheUsage = { used: number; limit: number };

/** The limits the settings screen offers; 1 GB is the default (`DEFAULT_DISK_LIMIT_MB`). */
export const CACHE_LIMITS_MB = [256, 512, 1024, 2048, 5120];

const MEGABYTE = 1024 * 1024;

export function megabytes(bytes: number): number {
  return Math.round(bytes / MEGABYTE);
}

/** Megabytes below a gigabyte, gigabytes with one decimal from there. */
export function formatBytes(bytes: number, locale: string): string {
  const gigabytes = bytes / (1024 * MEGABYTE);
  if (gigabytes >= 1) {
    return new Intl.NumberFormat(locale, { style: "unit", unit: "gigabyte", maximumFractionDigits: 1 }).format(gigabytes);
  }
  return new Intl.NumberFormat(locale, { style: "unit", unit: "megabyte", maximumFractionDigits: 0 }).format(
    bytes / MEGABYTE,
  );
}

export function cacheUsage(): Promise<CacheUsage> {
  return invoke<CacheUsage>("cache_status");
}

export async function setCacheLimit(limitMb: number): Promise<CacheUsage | SettingsChangeError> {
  try {
    return await invoke<CacheUsage>("set_cache_limit", { megabytes: limitMb });
  } catch (error) {
    if (isChangeError(error)) return error;
    throw error;
  }
}

/** Resolves to `"failed"` when some kept images could not be removed. */
export function clearCache(): Promise<CacheUsage | "failed"> {
  return invoke<CacheUsage>("clear_cache").catch((error: unknown) => {
    console.error("could not clear the image cache", error);
    return "failed" as const;
  });
}
