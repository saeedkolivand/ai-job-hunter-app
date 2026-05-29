import { expect, test } from '@playwright/test';

// Seed persisted preferences so the first-run onboarding wizard (a full-screen
// modal that would intercept clicks) does not appear.
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

test.describe('AI Job Hunter shell (mock client)', () => {
  test('boots the renderer into the dashboard', async ({ page }) => {
    const errors: string[] = [];
    page.on('pageerror', (e) => errors.push(e.message));

    await page.goto('/e2e.html');

    await expect(page.locator('#root')).not.toBeEmpty();
    await expect(page).toHaveURL(/\/(#.*)?$|\/$/);
    expect(errors, `page errors: ${errors.join('\n')}`).toEqual([]);
  });

  test('renders the sidebar navigation', async ({ page }) => {
    await page.goto('/e2e.html');
    const links = page.locator('a[href]');
    await expect.poll(async () => links.count()).toBeGreaterThan(3);
  });

  test('navigates to the Jobs route via the sidebar', async ({ page }) => {
    await page.goto('/e2e.html');
    await page.getByRole('link', { name: /jobs/i }).first().click();
    await expect(page).toHaveURL(/jobs/);
  });

  test('navigates to the Settings route via the sidebar', async ({ page }) => {
    await page.goto('/e2e.html');
    await page
      .getByRole('link', { name: /settings/i })
      .first()
      .click();
    await expect(page).toHaveURL(/settings/);
  });
});
