/**
 * App-shell frosting for portaled overlays.
 *
 * WebView2 does not reliably composite a portaled overlay's `backdrop-filter`,
 * so the frosted-glass effect comes from blurring the in-flow app shell instead
 * (the `.modal-blur-active` rule in the app CSS). The class is ref-counted so
 * stacked overlays — a drawer that opens a confirm modal, say — keep the blur
 * until the LAST one closes. Toggling a body class keeps this app-agnostic.
 *
 * Every portaled surface (`ModalShell`, `Drawer`, …) must call this or its glass
 * silently degrades to a flat scrim on the shipping platform.
 */
let openOverlayCount = 0;

export function setModalBlur(active: boolean): void {
  if (typeof document === 'undefined') return;
  if (active) {
    openOverlayCount += 1;
    document.body.classList.add('modal-blur-active');
  } else {
    openOverlayCount = Math.max(0, openOverlayCount - 1);
    if (openOverlayCount === 0) document.body.classList.remove('modal-blur-active');
  }
}

/** Test-only introspection of the ref count. */
export function modalBlurCount(): number {
  return openOverlayCount;
}
