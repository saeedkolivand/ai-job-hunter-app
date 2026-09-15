/**
 * Résumé-attach injected entry (compiled to `attach-file.js`).
 *
 * Injected on the user's "Attach résumé to this page" click via
 * `chrome.scripting.executeScript({ files: ['attach-file.js'] })` — NOT a
 * persistently registered content script (`activeTab` + `scripting`, same
 * pattern as `fill.ts`).
 *
 * It only exposes {@link runAttachFile} on the page's isolated-world global;
 * the background then calls it with the decoded résumé bytes via a second
 * `executeScript({ func, args })`. Splitting the data-passing out of `files`
 * injection keeps the bytes (the user's own résumé) out of any stored/
 * registered surface — they are handed in transiently for the one call and
 * never persisted.
 */

import { ATTACH_FILE_GLOBAL, type AttachFileResult, runAttachFile } from './lib/attach-file';

// Expose the attacher on the isolated-world global under a namespaced key so
// the background's second `executeScript({ func })` can invoke it with the
// decoded résumé bytes.
(
  globalThis as unknown as Record<
    string,
    (base64: string, filename: string, mimeType: string) => AttachFileResult
  >
)[ATTACH_FILE_GLOBAL] = runAttachFile;

// Ensure this file is treated as an ES module.
export {};
