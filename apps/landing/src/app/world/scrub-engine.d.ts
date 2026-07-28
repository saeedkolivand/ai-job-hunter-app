// Type declaration for the vendored scrub-engine.js (framework-agnostic vanilla
// JS scroll-scrubbed camera-flight engine — see that file's header comment for
// the full config shape). `config: object` (not `Record<string, unknown>`) is
// deliberate: the JS module accepts any plain config object, and `object`
// accepts WorldClient.tsx's typed WORLD_CONFIG (world-config.ts) without
// TS demanding a matching index signature on that named interface.
// Returns a disposer: it removes every window listener, cancels the rAF loop,
// releases each clip's decoder + blob URL and empties the container, so the
// engine can be mounted again (StrictMode's double-invoke, a route change, a
// test). See ADR-0019's deviation log — upstream returns nothing.
export function mountScrollWorld(container: HTMLElement, config: object): () => void;
