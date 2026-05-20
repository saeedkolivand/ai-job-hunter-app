import type { Page } from 'playwright';

/**
 * LinkedIn Easy Apply — SCAFFOLD ONLY.
 *
 * What it does today:
 *   1. Opens the posting URL in the persistent LinkedIn context.
 *   2. Detects whether the role offers Easy Apply.
 *   3. Walks through the apply modal pages, attempting to autofill known
 *      common fields (name, email, phone).
 *   4. Pauses at the final review step.
 *
 * What it does NOT do yet:
 *   - Custom-question detection / answering.
 *   - File uploads (resume PDF) — selectors vary by employer.
 *   - Cover letter insertion in the LinkedIn-side textbox.
 *   - Actually click "Submit application".
 *
 * Submitting applications is intentionally gated behind `ctx.autoSubmit`.
 * Even with that flag, do not enable for users in production without a
 * confirmation UI inside the app.
 */
import { type Applier, type ApplyContext, type ApplyResult, BaseApplier } from '../base.js';

export class LinkedInApplier extends BaseApplier implements Applier {
  readonly boardId = 'linkedin';
  readonly displayName = 'LinkedIn Easy Apply';

  async apply(postingUrl: string, ctx: ApplyContext): Promise<ApplyResult> {
    let stage = 'open';
    return ctx.browser.withPage(
      async (page): Promise<ApplyResult> => {
        try {
          await page.goto(postingUrl, { waitUntil: 'domcontentloaded', timeout: 30_000 });
          ctx.onStep?.({ stage, ok: true });
          ctx.onProgress?.(0.1, stage);

          stage = 'detect-easy-apply';
          const easyBtn = page
            .locator('button:has-text("Easy Apply"), button.jobs-apply-button')
            .first();
          const visible = await easyBtn.isVisible({ timeout: 6_000 }).catch(() => false);
          if (!visible) {
            ctx.onStep?.({ stage, ok: false, note: 'Easy Apply not available on this posting' });
            return { ok: false, stage, submitted: false, url: postingUrl, note: 'no-easy-apply' };
          }
          await easyBtn.click();
          ctx.onProgress?.(0.25, stage);

          stage = 'autofill';
          await this.fillContactFields(page, ctx);
          ctx.onProgress?.(0.55, stage);

          stage = 'navigate-pages';
          for (let i = 0; i < 6; i++) {
            if (ctx.signal.aborted) break;
            const next = page.locator('button:has-text("Next"), button:has-text("Weiter")').first();
            const review = page
              .locator('button:has-text("Review"), button:has-text("Überprüfen")')
              .first();
            if (await review.isVisible({ timeout: 1_500 }).catch(() => false)) {
              await review.click();
              break;
            }
            if (await next.isVisible({ timeout: 1_500 }).catch(() => false)) {
              await next.click();
              await page.waitForTimeout(400 + Math.random() * 300);
              await this.fillContactFields(page, ctx);
            } else {
              break;
            }
          }
          ctx.onProgress?.(0.85, stage);

          stage = 'final-review';
          // Stop here unless the user explicitly opted in.
          if (!ctx.autoSubmit) {
            ctx.onStep?.({ stage, ok: true, note: 'Stopped at review (autoSubmit=false)' });
            return { ok: true, stage, submitted: false, url: postingUrl, note: 'review-pending' };
          }

          stage = 'submit';
          const submit = page
            .locator('button:has-text("Submit application"), button:has-text("Bewerbung absenden")')
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

  /** Fill any visible common LinkedIn fields. Safe to call multiple times. */
  private async fillContactFields(page: Page, ctx: ApplyContext): Promise<void> {
    if (!ctx.credentials) return;
    const phoneInput = page.locator('input[id*="phoneNumber"]').first();
    if (await phoneInput.isVisible({ timeout: 800 }).catch(() => false)) {
      const v = await phoneInput.inputValue().catch(() => '');
      if (!v) await phoneInput.fill('').catch(() => {});
    }
    // Cover letter textarea (some employers add one)
    if (ctx.coverLetter) {
      const ta = page.locator('textarea').first();
      if (await ta.isVisible({ timeout: 800 }).catch(() => false)) {
        await ta.fill(ctx.coverLetter).catch(() => {});
      }
    }
  }
}
