// Factory bodies for the `vi.mock` stubs in the StepTemplate suites. Kept apart from
// ./test-support (which imports the subject) so a lazily-loaded factory never waits on
// the module that is itself waiting on the mock.

// `t` is identity EXCEPT for the two caption keys the i18n-resolution test
// exercises — those map to distinguishable copy so that test can tell a
// resolved translation apart from the raw key (see #965 R7).
const CAPTION_TRANSLATIONS: Record<string, string> = {
  'aiGenerate.templateCaption.classic': 'Best for maximum ATS safety.',
  'aiGenerate.templateCaption.jake': 'Best for a dense, classic single column.',
};
export const translationsMock = {
  useTranslation: () => ({ t: (key: string) => CAPTION_TRANSLATIONS[key] ?? key }),
};

// TEMPLATE_PREVIEWS, COVER_TEMPLATE_PREVIEWS, and TEMPLATE_CAPTIONS use
// import.meta.glob — stub them all so no Vite transform is needed in jsdom.
// Distinct non-empty URLs per template id so thumbnail-source tests can assert
// which preview set is used.
export function samplesMock() {
  const ids = [
    'classic',
    'swiss-minimal',
    'academic',
    'atelier',
    'meridian',
    'throughline',
    'portrait',
    'lebenslauf',
  ] as const;
  const resumePreviews = Object.fromEntries(ids.map((id) => [id, `resume-${id}.png`]));
  const coverPreviews = Object.fromEntries(ids.map((id) => [id, `cover-${id}.svg`]));
  return {
    TEMPLATE_PREVIEWS: resumePreviews as Record<string, string>,
    COVER_TEMPLATE_PREVIEWS: coverPreviews as Record<string, string>,
    // Real captions are i18n keys, not display text — mirrors samples.ts.
    TEMPLATE_CAPTIONS: {
      classic: 'aiGenerate.templateCaption.classic',
      jake: 'aiGenerate.templateCaption.jake',
    } as Record<string, string>,
  };
}
