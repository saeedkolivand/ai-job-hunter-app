import AxeBuilder from '@axe-core/playwright';
import { test } from '@playwright/test';

// Mirror app.spec's onboarding-skip seeding so the first-run wizard (a modal
// that would intercept everything) does not appear during the scan.
test.beforeEach(async ({ page }) => {
  await page.addInitScript(() => {
    localStorage.setItem(
      'ai-job-hunter-preferences',
      JSON.stringify({
        state: {
          version: 3,
          language: 'en',
          outputTone: 'professional',
          performanceMode: 'balanced',
          promptQuality: 'auto',
          debugMode: false,
          onboardingCompleted: true,
        },
        version: 3,
      })
    );
  });
});

// Advisory accessibility scan — reports axe-core violations without failing the
// suite. Tighten to an assertion once the home view is clean.
test('a11y: axe scan of the home view (advisory)', async ({ page }) => {
  await page.goto('/e2e.html');
  const { violations } = await new AxeBuilder({ page }).analyze();
  for (const v of violations) {
    console.warn(`[axe] ${v.impact ?? 'n/a'} — ${v.id}: ${v.help} (${v.nodes.length} node(s))`);
  }
  console.warn(`[axe] ${violations.length} violation rule(s) found (advisory, non-blocking).`);
});
