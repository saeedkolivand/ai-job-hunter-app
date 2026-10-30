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

  for (const height of [540, 640, 720, 900]) {
    test(`keeps the sidebar footer accessible at ${height}px height`, async ({ page }) => {
      await page.setViewportSize({ width: 1280, height });
      await page.addInitScript(() => {
        const key = 'ai-job-hunter-preferences';
        const prefs = JSON.parse(localStorage.getItem(key) ?? '{}');
        prefs.state.userName = 'Test User';
        localStorage.setItem(key, JSON.stringify(prefs));
      });
      await page.goto('/e2e.html');
      const sidebar = page.locator('aside.app-sidebar');
      const identity = sidebar.getByText('Test User', { exact: true });
      const settings = sidebar.getByRole('link', { name: 'Settings', exact: true });
      const help = sidebar.getByRole('link', { name: /Help.*Support/ });
      const version = sidebar.getByRole('button', { name: /^v\d+\./ });
      await expect(identity).toBeInViewport();
      await expect(settings).toBeInViewport();
      await expect(help).toBeInViewport();
      await expect(version).toBeInViewport();
      const navigation = sidebar.locator('nav').first();
      if (height <= 640) {
        expect(await navigation.evaluate((el) => el.scrollHeight > el.clientHeight)).toBe(true);
      }
      const monitoring = navigation.getByRole('link', { name: 'Monitoring', exact: true });
      await monitoring.scrollIntoViewIfNeeded();
      await expect(monitoring).toBeInViewport();
      if (height <= 640) {
        expect(await navigation.evaluate((el) => el.scrollTop)).toBeGreaterThan(0);
      }
      await expect(help).toBeInViewport();
      await expect(version).toBeInViewport();
      await settings.click();
      await expect(page).toHaveURL(/settings/);
      await expect(identity).toBeInViewport();
    });
  }

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
