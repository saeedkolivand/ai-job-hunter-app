import { readdirSync, readFileSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

import { defineConfig, type InlineConfig, type Plugin } from 'vite';

import { type BrowserTarget, buildManifest } from './src/manifest.ts';

const here = dirname(fileURLToPath(import.meta.url));
const srcDir = resolve(here, 'src');
const iconsDir = resolve(srcDir, 'icons');

/** Selected from the `BROWSER` env (`build:chrome` / `build:firefox`). */
const target: BrowserTarget = process.env.BROWSER === 'firefox' ? 'firefox' : 'chrome';

/** Per-target output dir: apps/extension/dist/<target>. */
const outDir = resolve(here, 'dist', target);

/**
 * Emit the resolved manifest + copy the static icons into the build output.
 * Runs in `generateBundle` (purely static asset assembly — no remote code, no
 * runtime codegen) so the same plugin serves whichever `--outDir` the CLI sets.
 */
function webExtensionAssets(): Plugin {
  return {
    name: 'ajh-webext-assets',
    generateBundle() {
      this.emitFile({
        type: 'asset',
        fileName: 'manifest.json',
        source: `${JSON.stringify(buildManifest(target), null, 2)}\n`,
      });
      for (const file of readdirSync(iconsDir)) {
        if (!file.endsWith('.png')) continue;
        this.emitFile({
          type: 'asset',
          fileName: `icons/${file}`,
          source: new Uint8Array(readFileSync(resolve(iconsDir, file))),
        });
      }
      // The popup display font (Patrick Hand, OFL, vendored under src/fonts) is
      // emitted automatically by Vite's CSS url() asset pipeline into the build
      // root, and popup.css is rewritten to reference it — no manual copy needed,
      // and still no remote fetch.
    },
  };
}

/**
 * `content.ts` (Scan-mode DOM capture), `fill.ts` (assisted autofill),
 * `capture.ts` (answers capture), `capture-questions.ts` (questions-mode
 * collector), `answer-fill.ts` (single-field answer fill), `answer-replace.ts`
 * (single-field answer REPLACE, extension PR 11's rewrite Accept/Restore),
 * and `probe-fields.ts` (the popup's fillable-fields probe) are ALL injected
 * via `chrome.scripting.executeScript({ files: [...] })`, which runs as a
 * CLASSIC script (no ES modules) — so each compiled bundle must carry ZERO
 * `import` statements. Since PR 5 of the extension roadmap, they genuinely
 * share runtime code (`lib/field-signal.ts`, via `lib/autofill.ts` and
 * `lib/answers-capture.ts`; PR 6 adds `lib/answer-fill.ts`, which itself
 * imports `lib/answers-capture.ts`'s `locateQuestionField`; PR 11 adds
 * `lib/answer-fill.ts`'s `replaceFilledField`, which `answer-replace.ts`
 * imports the same way; `content.ts` imports `lib/field-signal.ts`'s
 * `isHidden` directly, mirroring `capture.ts`'s convention): if built
 * together with the main multi-entry pass above, Rollup's default cross-entry
 * chunking would hoist shared modules into a `chunks/*.js` file that multiple
 * of these would then `import` — breaking classic-script injection (verified
 * empirically: entries sharing a static import always get split into a
 * shared chunk in one Rollup pass, even with no other config).
 *
 * The fix: build EACH in its OWN isolated single-entry Rollup pass (this
 * plugin's `closeBundle`, which runs after the main bundle above has already
 * written background/popup + the manifest/icons). A single-entry pass has
 * nothing to hoist against, so the shared helpers are INLINED into each file
 * instead. `emptyOutDir: false` so neither pass wipes what the other (or the
 * main build) already wrote.
 *
 * MINIFICATION IS OFF for this pass, and that is load-bearing. Four of these
 * scripts (`content`, `capture`, `capture-questions`, `probe-fields`) answer
 * the background by COMPLETION VALUE — `executeScript({ files })` hands back
 * whatever the file's LAST STATEMENT evaluates to — and a minifier is entitled
 * to rewrite away a trailing pure expression whose value it believes nobody
 * reads. Vite 8's default minifier (oxc — `build.minify: true` resolves to
 * `'oxc'`) did exactly that to `capture.ts`, folding
 * `(() => ({ answers: a(document), filled: b(document) }))()` into
 * `a(document),b(document);`: the completion value became the last CALL's
 * array, `background.ts`'s `isCaptureResult` rejected it, and "Save my answers
 * from this page" failed with "Could not read the answers on this page." on
 * every page — in the store build, the release zip, and a fresh local build.
 * `content.js` and `capture-questions.js` survived only incidentally (a
 * try/catch body, a bare array return), so keeping the minifier off the whole
 * pass — rather than hand-picking trailing expressions this minifier version
 * happens not to fold — is what removes the CLASS instead of the instance. A
 * completion value is a contract no minifier can see, so it does not get to
 * optimise it; these files are tens of KB each, and readable injected source is
 * what AMO source-code review wants anyway. `src/build-output.test.ts`
 * evaluates the BUILT files and asserts their completion values, so re-enabling
 * minification here fails a test instead of a release.
 */

/** Every classic script injected via `executeScript({ files })`, each built in
 *  its own isolated single-entry pass. */
export const INJECTED_ENTRIES = [
  'content',
  'fill',
  'capture',
  'capture-questions',
  'answer-fill',
  'answer-replace',
  'submit-watch',
  'probe-fields',
] as const;

/**
 * The EXACT options one injected entry is built with. Exported alongside
 * {@link INJECTED_ENTRIES} so `src/build-output.test.ts` can produce the real
 * shipped artifact into a temp dir — a completion-value guard that rebuilt with
 * its own hand-written options, or read a stale `dist/`, would not be testing
 * what the store gets.
 */
export function injectedEntryConfig(name: string, entryOutDir: string): InlineConfig {
  return {
    configFile: false,
    root: srcDir,
    logLevel: 'warn',
    build: {
      outDir: entryOutDir,
      emptyOutDir: false,
      target: 'es2022',
      modulePreload: false,
      // See the comment block above: these scripts communicate by completion
      // value, which a minifier may legally rewrite away.
      minify: false,
      rollupOptions: {
        input: { [name]: resolve(srcDir, `${name}.ts`) },
        output: { entryFileNames: '[name].js', format: 'es' },
      },
    },
  };
}

function injectedEntries(): Plugin {
  return {
    name: 'ajh-injected-classic-scripts',
    apply: 'build',
    async closeBundle() {
      const { build } = await import('vite');
      for (const name of INJECTED_ENTRIES) {
        await build(injectedEntryConfig(name, outDir));
      }
    },
  };
}

export default defineConfig({
  root: srcDir,
  // Relative base so the popup HTML references ./popup.js / ./popup.css from the
  // extension root rather than an absolute path the packaged extension can't use.
  base: './',
  plugins: [webExtensionAssets(), injectedEntries()],
  build: {
    outDir,
    emptyOutDir: true,
    target: 'es2022',
    modulePreload: false,
    rollupOptions: {
      input: {
        background: resolve(srcDir, 'background.ts'),
        popup: resolve(srcDir, 'popup.html'),
      },
      output: {
        // Stable, manifest-referenced filenames at the dist root.
        entryFileNames: '[name].js',
        chunkFileNames: 'chunks/[name].js',
        assetFileNames: '[name][extname]',
        format: 'es',
      },
    },
  },
  // Keep the bundle reviewable (AMO requires readable source).
  esbuild: {
    legalComments: 'none',
  },
});
