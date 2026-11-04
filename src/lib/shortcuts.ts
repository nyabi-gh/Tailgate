/**
 * Keyboard shortcuts go through one window listener that knows which layer is on top: a modal
 * sheet over the viewer over the app. Keys typed into a sheet over the viewer must not reach the
 * viewer (`d` would download), nor the app behind either.
 */
export type Layer = "global" | "viewer" | "app";

/** Returns whether it took the key. */
type Handler = (event: KeyboardEvent) => boolean;

const handlers = new Map<Layer, Handler>();

/** One handler per layer; the viewer's exists only while the viewer is open. */
export function onKeys(layer: Layer, handler: Handler): () => void {
  handlers.set(layer, handler);
  return () => {
    if (handlers.get(layer) === handler) handlers.delete(layer);
  };
}

export function dispatchKey(event: KeyboardEvent) {
  if (handlers.get("global")?.(event)) return;
  // An open sheet keeps its keys, Escape included, which closes it and nothing under it.
  if (document.querySelector("dialog:modal") !== null) return;
  (handlers.get("viewer") ?? handlers.get("app"))?.(event);
}

export function isEditable(target: EventTarget | null): boolean {
  return (
    target instanceof HTMLInputElement ||
    target instanceof HTMLTextAreaElement ||
    target instanceof HTMLSelectElement ||
    (target instanceof HTMLElement && target.isContentEditable)
  );
}
