// i18next-parser — extracts t() keys from the renderer and reconciles them with
// the locale files. Run advisorily in CI (see .github/workflows/quality.yml):
// if running it would change the locale JSON, there are missing/unused keys.
export default {
  locales: ['en', 'de'],
  input: ['apps/tauri/src/renderer/**/*.{ts,tsx}'],
  output: 'apps/tauri/src/renderer/i18n/locales/$LOCALE.json',
  keySeparator: '.',
  namespaceSeparator: ':',
  defaultNamespace: 'translation',
  sort: true,
  keepRemoved: false,
  createOldCatalogs: false,
  failOnWarning: false,
};
