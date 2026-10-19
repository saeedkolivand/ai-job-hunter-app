import { expect, test } from '@playwright/test';

import de from '../../../packages/translations/src/locales/de/translation.json';
import en from '../../../packages/translations/src/locales/en/translation.json';

const routes = [
  '/applications',
  '/jobs',
  '/analyze',
  '/ai-generate',
  '/build',
  '/documents',
  '/autopilot',
  '/best-matches',
  '/monitoring',
  '/support',
  '/settings',
  '/',
];

for (const viewport of [
  { width: 900, height: 540 },
  { width: 1280, height: 720 },
  { width: 1440, height: 900 },
]) {
  for (const language of ['en', 'de']) {
    test(`workspace routes ${viewport.width}x${viewport.height} ${language}`, async ({ page }) => {
      await page.setViewportSize(viewport);
      await page.addInitScript(
        ({ language }) => {
          localStorage.setItem(
            'ai-job-hunter-preferences',
            JSON.stringify({
              state: {
                userName: 'Test User',
                language,
                onboardingCompleted: true,
                contactPromptSeen: true,
              },
              version: 5,
            })
          );
        },
        { language }
      );
      const errors: string[] = [];
      page.on('pageerror', (error) => errors.push(error.message));
      await page.goto('/e2e.html');
      for (const route of routes) {
        const link = page.locator(`aside.app-sidebar a[href="${route}"]`).first();
        await link.scrollIntoViewIfNeeded();
        await link.click();
        await expect(page).toHaveURL(new RegExp(`${route === '/' ? '/' : route}/?$`));
        await expect(page.locator('main')).toBeVisible();
        expect((await page.locator('main').innerText()).trim().length, route).toBeGreaterThan(0);
        expect(errors, `${route}: ${errors.join('; ')}`).toEqual([]);
        if (route === '/settings') {
          const copy = language === 'de' ? de : en;
          for (const section of Object.values(copy.settings.sections)) {
            const button = page.locator('main aside').getByRole('button', {
              name: section.label,
              exact: true,
            });
            await button.scrollIntoViewIfNeeded();
            await button.click();
            await expect(button).toHaveAttribute('aria-current', 'page');
            await expect(
              page.locator('main').getByText(section.description, { exact: true })
            ).toBeVisible();
            expect(errors, section.label).toEqual([]);
          }
        }
      }
    });
  }
}
