export const isMac = navigator.userAgent.includes("Macintosh");

export const isWindows = navigator.userAgent.includes("Windows");

export const isTauri = "__TAURI_INTERNALS__" in window;

export const shortcutLabel = (key: string) => (isMac ? `⌘${key}` : `Ctrl ${key}`);
