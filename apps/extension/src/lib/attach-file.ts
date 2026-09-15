/**
 * Pure "attach a résumé File to the page's `type=file` field via
 * DataTransfer" logic (PR2 §C.3) — the fail-closed counterpart of
 * `lib/autofill.ts`'s text fill, for the one field type Fill never touches.
 *
 * Locates the target with the SAME `isResumeFileInput` heuristic
 * `submit-watch.ts` already uses (exported there, imported here — one
 * heuristic, two consumers, never a forked copy), builds a `File` from the
 * decoded export bytes, assigns it through a `DataTransfer`
 * (`input.files = dt.files` — the browser-sanctioned assignment path for a
 * file input), dispatches `input`+`change`, and — when a drop-zone ancestor
 * is detectable — an additional synthetic `drop` carrying the SAME
 * `DataTransfer` (some custom upload widgets listen for `drop`, not
 * `change`). Then VERIFIES by RE-READING `input.files[0]` (name + size)
 * rather than trusting the assignment — a custom widget can silently ignore
 * it, and this module fails closed (`{attached:false, reason}`) rather than
 * ever claiming success it did not confirm.
 *
 * Injected via `attach-file.ts` (compiled to `attach-file.js`, a classic
 * script — see `injected-entries.mjs`), mirroring `fill.ts`'s two-step
 * register-then-invoke pattern: the bytes are the user's own résumé, so they
 * cross the `executeScript` boundary transiently via a second call rather
 * than baked into the `files` injection.
 *
 * Pure DOM — no extension APIs — so it is unit-testable against a jsdom
 * document, given the minimal `DataTransfer`/`input.files` polyfill the test
 * file installs (jsdom implements neither for real).
 */

import { type AutofillSummary, renderSummaryOverlay } from './autofill';
import { isResumeFileInput } from './submit-watch';

/** Isolated-world global key `attach-file.ts` exposes the runner under. MUST
 *  match the literal duplicated in `background.ts` (kept a plain literal
 *  there, not imported — same discipline as `AUTOFILL_GLOBAL`). */
export const ATTACH_FILE_GLOBAL = '__ajhRunAttachFile';

/** The attach outcome — fail-closed on anything short of a confirmed re-read. */
export interface AttachFileResult {
  attached: boolean;
  filename?: string;
  byteLength?: number;
  reason?: string;
}

const NO_FIELD: AttachFileResult = {
  attached: false,
  reason: 'No résumé upload field found on this page.',
};

const AMBIGUOUS: AttachFileResult = {
  attached: false,
  reason: 'Found more than one résumé upload field on this page — attach it yourself.',
};

const DISABLED: AttachFileResult = {
  attached: false,
  reason: 'The résumé upload field on this page is disabled.',
};

const NOT_CONFIRMED: AttachFileResult = {
  attached: false,
  reason: 'Could not confirm the file attached — this page may use a custom upload widget.',
};

/** How many ancestor levels up from the file input to look for a drop-zone —
 *  bounded so an unrelated distant ancestor is never mistaken for one. */
const DROP_ZONE_SEARCH_DEPTH = 5;

/** Signal that names an element as a drag-drop target: a `class`/`id`
 *  containing "drop" (case-insensitive), or an explicit `data-dropzone`/
 *  inline `ondrop` attribute. Best-effort — a widget with none of these
 *  simply gets no synthetic `drop` (the `input`/`change` events already
 *  fired regardless). */
function isDropZone(el: Element): boolean {
  if (el.hasAttribute('ondrop') || el.hasAttribute('data-dropzone')) return true;
  const className = typeof el.className === 'string' ? el.className : '';
  return /drop/i.test(className) || /drop/i.test(el.id);
}

/** The nearest drop-zone ancestor of `el`, within
 *  {@link DROP_ZONE_SEARCH_DEPTH} levels, or `null` when none is detectable. */
function nearestDropZone(el: HTMLElement): HTMLElement | null {
  let current: HTMLElement | null = el.parentElement;
  for (let depth = 0; current && depth < DROP_ZONE_SEARCH_DEPTH; depth += 1) {
    if (isDropZone(current)) return current;
    current = current.parentElement;
  }
  return null;
}

/**
 * Attach `bytes` (as `filename`/`mimeType`) to `doc`'s résumé file input.
 * Fails closed on: no candidate, more than one candidate (never guesses
 * which), a disabled/readonly field, or a re-read that does not confirm the
 * assignment actually took.
 */
export function attachResumeFile(
  doc: Document,
  bytes: Uint8Array,
  filename: string,
  mimeType: string
): AttachFileResult {
  const candidates = Array.from(doc.querySelectorAll('input')).filter(isResumeFileInput);
  if (candidates.length === 0) return NO_FIELD;
  if (candidates.length > 1) return AMBIGUOUS;
  const [input] = candidates;
  if (!input) return NO_FIELD;
  if (input.matches(':disabled') || input.readOnly) return DISABLED;

  // `new Uint8Array(bytes)` (the array-like overload) always allocates a
  // fresh, concrete `ArrayBuffer` — unlike `bytes` itself, whose generic
  // buffer type (`ArrayBufferLike`, which also admits `SharedArrayBuffer`)
  // `File`'s `BlobPart` union does not accept directly.
  const file = new File([new Uint8Array(bytes)], filename, { type: mimeType });
  const dt = new DataTransfer();
  dt.items.add(file);
  input.files = dt.files;
  input.dispatchEvent(new Event('input', { bubbles: true }));
  input.dispatchEvent(new Event('change', { bubbles: true }));

  const dropZone = nearestDropZone(input);
  if (dropZone) {
    const drop = new Event('drop', { bubbles: true, cancelable: true });
    Object.defineProperty(drop, 'dataTransfer', { value: dt, configurable: true });
    dropZone.dispatchEvent(drop);
  }

  const confirmed = input.files?.[0];
  if (!confirmed || confirmed.name !== filename || confirmed.size !== bytes.byteLength) {
    return NOT_CONFIRMED;
  }
  return { attached: true, filename: confirmed.name, byteLength: confirmed.size };
}

/**
 * Decode a base64 string to raw bytes — called INSIDE the injected runner
 * below, never in the background (PR review round 2). Chrome JSON-serializes
 * `executeScript({ func, args })` arguments, so a `Uint8Array` arg would
 * arrive here as a plain `{"0":…}` object rather than a real typed array; a
 * base64 STRING is JSON-safe and survives that boundary intact, so
 * `background.ts` passes the raw base64 and this decodes it back to bytes
 * once it's already running on the page (never crossing the boundary as
 * bytes at all). `atob` + a byte-copy loop — no dependency; a private
 * duplicate of `background.ts`'s own `base64ToBytes` since this file is a
 * classic-script injection target and cannot import from it.
 */
function base64ToBytes(b64: string): Uint8Array {
  const binary = atob(b64);
  const bytes = new Uint8Array(binary.length);
  for (let i = 0; i < binary.length; i += 1) bytes[i] = binary.charCodeAt(i);
  return bytes;
}

/**
 * The injected entry-point: decode the base64 payload, attach, render the
 * shared summary overlay (see `AutofillSummary.attached`'s doc), and return
 * the result for the popup/panel. Kept side-effect-first so
 * `chrome.scripting.executeScript` gets a serializable return value — mirrors
 * `autofill.ts`'s `runAutofill`.
 */
export function runAttachFile(
  base64: string,
  filename: string,
  mimeType: string
): AttachFileResult {
  const bytes = base64ToBytes(base64);
  const result = attachResumeFile(document, bytes, filename, mimeType);
  const summary: AutofillSummary = {
    filled: [],
    nameSplit: null,
    filledNothing: false,
    attached: result.attached
      ? { filename: result.filename ?? filename, byteLength: result.byteLength ?? bytes.byteLength }
      : null,
  };
  renderSummaryOverlay(document, summary);
  return result;
}
