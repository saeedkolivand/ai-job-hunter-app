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

export default defineConfig({
  root: srcDir,
  // Relative base so the popup HTML references ./popup.js / ./popup.css from the
  // extension root rather than an absolute path the packaged extension can't use.
  base: './',
  plugins: [webExtensionAssets()],
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
