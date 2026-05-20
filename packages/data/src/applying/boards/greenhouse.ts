/**
 * Greenhouse applier.
 *
 * Public Greenhouse postings serve a straightforward HTML form at
 *   https://boards.greenhouse.io/{company}/jobs/{id}
 * The form always contains:
 *   #first_name, #last_name, #email, #phone, #resume (file input), #cover_letter
 *
 * No auth is required. CAPTCHA is sometimes injected (hCaptcha). When detected
 * we pause and let the user solve it.
 */
import { type Applier, type ApplyContext, type ApplyResult, BaseApplier } from '../base.js';

export class GreenhouseApplier extends BaseApplier implements Applier {
  readonly boardId = 'greenhouse';
  readonly displayName = 'Greenhouse';

  async apply(postingUrl: string, ctx: ApplyContext): Promise<ApplyResult> {
    let stage = 'open';
    return ctx.browser.withPage(
      async (page): Promise<ApplyResult> => {
        try {
          await page.goto(postingUrl, { waitUntil: 'domcontentloaded', timeout: 30_000 });
          ctx.onStep?.({ stage, ok: true });
          ctx.onProgress?.(0.1, stage);

          // Some Greenhouse pages put the form behind a separate "Apply for this Job" link.
          const applyLink = page.locator('a:has-text("Apply for this Job")').first();
          if (await applyLink.isVisible({ timeout: 2_000 }).catch(() => false)) {
            await applyLink.click();
            await page.waitForLoadState('domcontentloaded');
          }

          stage = 'detect-form';
          const form = page.locator('form[action*="apply"], #application_form, form').first();
          if (!(await form.isVisible({ timeout: 6_000 }).catch(() => false))) {
            ctx.onStep?.({ stage, ok: false, note: 'Application form not found' });
            return { ok: false, stage, submitted: false, url: postingUrl, note: 'form-not-found' };
          }
          ctx.onProgress?.(0.3, stage);

          stage = 'autofill';
          const c = ctx.credentials;
          if (c?.username) {
            // Username here is the user's email saved in credentials.
            await page.fill('#email', c.username).catch(() => {});
          }
          if (ctx.resumePath) {
            const file = page.locator('input[type="file"]').first();
            if (await file.count()) {
              await file.setInputFiles(ctx.resumePath).catch(() => {});
            }
          }
          if (ctx.coverLetter) {
            // Greenhouse cover-letter textarea, when present.
            const ta = page.locator('textarea[name*="cover_letter"], #cover_letter').first();
            if (await ta.isVisible({ timeout: 800 }).catch(() => false)) {
              await ta.fill(ctx.coverLetter).catch(() => {});
            }
          }
          ctx.onProgress?.(0.65, stage);

          stage = 'captcha-check';
          if (
            await page
              .locator('iframe[src*="hcaptcha"], .h-captcha, [data-sitekey]')
              .first()
              .isVisible()
              .catch(() => false)
          ) {
            ctx.onStep?.({ stage, ok: false, note: 'Captcha required — solve in window' });
            await page.waitForURL(/.+/, { timeout: 120_000 }).catch(() => {});
          }
          ctx.onProgress?.(0.85, stage);

          stage = 'final-review';
          if (!ctx.autoSubmit) {
            ctx.onStep?.({ stage, ok: true, note: 'Stopped at review (autoSubmit=false)' });
            return { ok: true, stage, submitted: false, url: postingUrl, note: 'review-pending' };
          }

          stage = 'submit';
          const submit = page
            .locator('button[type="submit"], input[type="submit"], button:has-text("Submit")')
            .first();
          if (await submit.isVisible({ timeout: 4_000 }).catch(() => false)) {
            await submit.click();
            ctx.onProgress?.(1, stage);
            ctx.onStep?.({ stage, ok: true });
            return { ok: true, stage, submitted: true, url: postingUrl };
          }
          return { ok: false, stage, submitted: false, url: postingUrl, note: 'submit-not-found' };
        } catch (err) {
          const note = err instanceof Error ? err.message : String(err);
          ctx.onStep?.({ stage, ok: false, note });
          return { ok: false, stage, submitted: false, url: postingUrl, note };
        }
      },
      { persistentStateDir: ctx.storageStatePath, boardId: this.boardId }
    );
  }
}
