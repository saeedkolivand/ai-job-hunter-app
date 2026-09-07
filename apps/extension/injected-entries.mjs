// The single source of truth for the classic scripts injected via
// `chrome.scripting.executeScript({ files: [...] })`.
//
// WHY these are special — and why the list has to be shared rather than copied —
// is the long comment block above `injectedEntryConfig` in `vite.config.mts`:
// each is built in its own isolated single-entry Rollup pass (so no shared chunk
// is hoisted out and `import`ed, which classic-script injection cannot load) and
// each is emitted UNMINIFIED (several answer the background by completion value,
// which a minifier is entitled to fold away). Read that before changing this.
//
// Plain `.mjs`, not part of `vite.config.mts`, because three consumers need it
// and two of them are zero-dependency Node scripts that cannot load TypeScript:
//
//   - `vite.config.mts`             — builds one isolated pass per entry
//   - `scripts/package.mjs`         — refuses to package if a built entry
//                                     contains an `import`/`export`
//   - `scripts/source-archive.mjs`  — names them in the AMO source-build README,
//                                     which is what tells a Mozilla reviewer why
//                                     some files in the bundle are not minified
//
// Every hand-copied version of this list has drifted: `package.mjs` guarded 5 of
// the 9, and the AMO README omitted `submit-watch.js` outright. Add an entry
// here and all three follow.
export const INJECTED_ENTRIES = [
  'content', // Scan-mode DOM capture
  'fill', // assisted autofill
  'capture', // answers capture
  'capture-questions', // questions-mode collector
  'capture-rows', // the ADR-044 answer-rows scan
  'answer-fill', // single-field answer fill
  'answer-replace', // single-field answer REPLACE (rewrite Accept/Restore)
  'submit-watch', // post-submit application watcher
  'probe-fields', // the popup's fillable-fields probe
];

/** The built filenames, at the root of `dist/<target>/`. */
export const INJECTED_SCRIPT_FILES = INJECTED_ENTRIES.map((name) => `${name}.js`);
