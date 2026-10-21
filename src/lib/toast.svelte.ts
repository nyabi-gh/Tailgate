/** A short notice at the bottom of the window, optionally with one action. */
export type Toast = { id: number; text: string; action?: { label: string; run: () => void } };

const VISIBLE_MS = 4000;
/** Long enough to reach the action button. */
const VISIBLE_WITH_ACTION_MS = 7000;

let toasts = $state<Toast[]>([]);
let nextId = 0;

export function currentToasts(): Toast[] {
  return toasts;
}

export function notify(text: string, action?: Toast["action"]) {
  const toast = { id: nextId++, text, action };
  toasts = [...toasts.slice(-2), toast];
  setTimeout(() => dismiss(toast.id), action ? VISIBLE_WITH_ACTION_MS : VISIBLE_MS);
}

export function dismiss(id: number) {
  toasts = toasts.filter((toast) => toast.id !== id);
}
