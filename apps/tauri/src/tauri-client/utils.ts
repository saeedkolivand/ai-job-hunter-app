// Registers a Tauri event listener and returns a sync unsubscribe handle.
export function asyncUnsub(setup: () => Promise<() => void>): () => void {
  let cancel: (() => void) | null = null;
  let cancelled = false;
  setup()
    .then((fn) => {
      if (cancelled) fn();
      else cancel = fn;
    })
    .catch(console.error);
  return () => {
    cancelled = true;
    cancel?.();
  };
}
