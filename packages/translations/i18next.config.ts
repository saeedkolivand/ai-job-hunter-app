import { defineConfig } from 'i18next-cli';

// i18next-cli — successor to the deprecated i18next-parser (#2). Extracts t()
// keys from the renderer and reconciles them with the locale files. Run
// advisorily in CI (see .github/workflows/quality.yml): if running it would add
// keys, there are t() calls with no translation.
//
// removeUnusedKeys is OFF on purpose: the renderer uses dynamic keys a static
// extractor can't see (e.g. t(`models.tier.${tier}`), t(`analyze.groups.${id}`),
// t(`aiGenerate.wizard.emphasis.${id}.label`)). Pruning would falsely delete
// them, so the check only flags MISSING keys — the reliable signal — and stays a
// clean no-op when the locales are in sync.
export default defineConfig({
  locales: ['en', 'de'],
  extract: {
    input: ['../../apps/desktop/src/renderer/**/*.{ts,tsx}'],
    // Per-language namespace files (the {{namespace}} segment gives a FLAT root,
    // no "translation" wrapper) — the app loads each as the `translation` ns.
    output: 'src/locales/{{language}}/{{namespace}}.json',
    keySeparator: '.',
    nsSeparator: ':',
    defaultNS: 'translation',
    primaryLanguage: 'en',
    sort: true,
    removeUnusedKeys: false,
    indentation: 2,
    // Keep every existing translation; only fill a genuinely-new key with its key
    // path (i18next-parser style). Without this the primary language is rewritten
    // from code defaults — which we don't have inline — blanking all English text.
    defaultValue: (_key, _ns, _lng, value) => value ?? _key,
  },
});
