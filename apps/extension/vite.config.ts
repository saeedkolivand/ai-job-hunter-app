import { readdirSync, readFileSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

import { defineConfig, type Plugin } from 'vite';

import { type BrowserTarget, buildManifest } from './src/manifest';

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
 * `fill.ts` (assisted autofill), `capture.ts` (answers capture),
 * `capture-questions.ts` (questions-mode collector), and `answer-fill.ts`
 * (single-field answer fill) are ALL injected via
 * `chrome.scripting.executeScript({ files: [...] })`, which runs as a CLASSIC
 * script (no ES modules) — so each compiled bundle must carry ZERO `import`
 * statements. Since PR 5 of the extension roadmap, they genuinely share
 * runtime code (`lib/field-signal.ts`, via `lib/autofill.ts` and
 * `lib/answers-capture.ts`; PR 6 adds `lib/answer-fill.ts`, which itself
 * imports `lib/answers-capture.ts`'s `locateQuestionField`): if built
 * together with the main multi-entry pass above, Rollup's default cross-entry
 * chunking would hoist shared modules into a `chunks/*.js` file that multiple
 * of these would then `import` — breaking classic-script injection (verified
 * empirically: entries sharing a static import always get split into a
 * shared chunk in one Rollup pass, even with no other config).
 *
 * The fix: build EACH in its OWN isolated single-entry Rollup pass (this
 * plugin's `closeBundle`, which runs after the main bundle above has already
 * written background/content/popup + the manifest/icons). A single-entry
 * pass has nothing to hoist against, so the shared helpers are INLINED into
 * each file instead. `emptyOutDir: false` so neither pass wipes what the
 * other (or the main build) already wrote.
 */
function injectedEntries(): Plugin {
  return {
    name: 'ajh-injected-classic-scripts',
    apply: 'build',
    async closeBundle() {
      const { build } = await import('vite');
      for (const name of ['fill', 'capture', 'capture-questions', 'answer-fill']) {
        await build({
          configFile: false,
          root: srcDir,
          logLevel: 'warn',
          build: {
            outDir,
            emptyOutDir: false,
            target: 'es2022',
            modulePreload: false,
            rollupOptions: {
              input: { [name]: resolve(srcDir, `${name}.ts`) },
              output: { entryFileNames: '[name].js', format: 'es' },
            },
          },
          esbuild: { legalComments: 'none' },
        });
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
        content: resolve(srcDir, 'content.ts'),
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
