import type { Page } from 'playwright';

/**
 * Indeed "Apply with Indeed" / Easy Apply — scaffold.
 *
 * Indeed's apply flow splits into three shapes:
 *   A) Indeed-hosted "Apply now" with a single modal (simple cases).
 *   B) Indeed Resume Apply (`/apply?jk=…`) with multi-step questions.
 *   C) External redirect to the employer ATS (we deliberately do NOT follow
 *      these — that's the ATS scraper's job).
 *
 * This applier handles (A) and best-effort (B). Submission is gated behind
 * `ctx.autoSubmit`. CAPTCHAs / human challenges pause the flow with a 2-min
 * wait window so the user can resolve them in-browser.
 */
import { type Applier, type ApplyContext, type ApplyResult, BaseApplier } from '../base.js';

export class IndeedApplier extends BaseApplier implements Applier {
  readonly boardId = 'indeed';
  readonly displayName = 'Indeed Easy Apply';

  async apply(postingUrl: string, ctx: ApplyContext): Promise<ApplyResult> {
    let stage = 'open';
    return ctx.browser.withPage(
      async (page): Promise<ApplyResult> => {
        try {
          await page.goto(postingUrl, { waitUntil: 'domcontentloaded', timeout: 30_000 });
          ctx.onStep?.({ stage, ok: true });
          ctx.onProgress?.(0.1, stage);

          // Captcha detection
          if (
            await page
              .locator('text=Verify you are human')
              .first()
              .isVisible()
              .catch(() => false)
          ) {
            ctx.onStep?.({ stage: 'captcha', ok: false, note: 'Solve in the browser window' });
            await page.waitForURL(/indeed\.com/, { timeout: 120_000 }).catch(() => {});
          }

          stage = 'detect-apply';
          const applyBtn = page
            .locator(
              'button:has-text("Apply now"), button[id^="indeedApplyButton"], #indeedApplyButton'
            )
            .first();
          if (!(await applyBtn.isVisible({ timeout: 6_000 }).catch(() => false))) {
            ctx.onStep?.({
              stage,
              ok: false,
              note: 'No Indeed Apply on this posting (likely external)',
            });
            return { ok: false, stage, submitted: false, url: postingUrl, note: 'external-apply' };
          }
          await applyBtn.click();
          ctx.onProgress?.(0.25, stage);

          stage = 'autofill';
          await this.fillCommonFields(page, ctx);
          ctx.onProgress?.(0.55, stage);

          stage = 'navigate-pages';
          for (let i = 0; i < 8; i++) {
            if (ctx.signal.aborted) break;
            const continueBtn = page
              .locator(
                'button:has-text("Continue"), button:has-text("Weiter"), button[data-testid*="IndeedApply"]'
              )
              .first();
            const reviewBtn = page
              .locator('button:has-text("Review"), button:has-text("Submit your application")')
              .first();
            if (await reviewBtn.isVisible({ timeout: 1_500 }).catch(() => false)) break;
            if (await continueBtn.isVisible({ timeout: 1_500 }).catch(() => false)) {
              await continueBtn.click();
              await page.waitForTimeout(500 + Math.random() * 400);
              await this.fillCommonFields(page, ctx);
            } else {
              break;
            }
          }
          ctx.onProgress?.(0.85, stage);

          stage = 'final-review';
          if (!ctx.autoSubmit) {
            ctx.onStep?.({ stage, ok: true, note: 'Stopped at review (autoSubmit=false)' });
            return { ok: true, stage, submitted: false, url: postingUrl, note: 'review-pending' };
          }

          stage = 'submit';
          const submit = page
            .locator(
              'button:has-text("Submit your application"), button:has-text("Bewerbung senden")'
            )
            .first();
          if (await submit.isVisible({ timeout: 4_000 }).catch(() => false)) {
            await submit.click();
            ctx.onProgress?.(1, stage);
            ctx.onStep?.({ stage, ok: true });
            return { ok: true, stage, submitted: true, url: postingUrl };
          }
          return {
            ok: false,
            stage,
            submitted: false,
            url: postingUrl,
            note: 'submit-button-not-found',
          };
        } catch (err) {
          const note = err instanceof Error ? err.message : String(err);
          ctx.onStep?.({ stage, ok: false, note });
          return { ok: false, stage, submitted: false, url: postingUrl, note };
        }
      },
      { persistentStateDir: ctx.storageStatePath, boardId: this.boardId }
    );
  }

  private async fillCommonFields(page: Page, ctx: ApplyContext): Promise<void> {
    if (ctx.coverLetter) {
      const ta = page.locator('textarea[id*="coverletter"], textarea[name*="cover"]').first();
      if (await ta.isVisible({ timeout: 800 }).catch(() => false)) {
        await ta.fill(ctx.coverLetter).catch(() => {});
      }
    }
  }
}
