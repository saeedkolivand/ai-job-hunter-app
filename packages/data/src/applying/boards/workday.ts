/**
 * Workday applier — scaffold.
 *
 * Workday is per-tenant. The apply flow typically requires creating an account
 * on each tenant ("Apply" → email → password → multi-page form). Because the
 * credential model is per-tenant, we currently only walk the flow up to the
 * sign-in screen and stop. Once the user signs in once via the visible window,
 * the persistent context reuses that session for subsequent applies on the
 * same tenant.
 *
 * This implementation is intentionally conservative: it never submits.
 */
import { type Applier, type ApplyContext, type ApplyResult, BaseApplier } from '../base.js';

export class WorkdayApplier extends BaseApplier implements Applier {
  readonly boardId = 'workday';
  readonly displayName = 'Workday';

  async apply(postingUrl: string, ctx: ApplyContext): Promise<ApplyResult> {
    let stage = 'open';
    return ctx.browser.withPage(
      async (page): Promise<ApplyResult> => {
        try {
          await page.goto(postingUrl, { waitUntil: 'domcontentloaded', timeout: 30_000 });
          ctx.onStep?.({ stage, ok: true });
          ctx.onProgress?.(0.2, stage);

          stage = 'detect-apply';
          const applyBtn = page
            .locator(
              'a:has-text("Apply"), button:has-text("Apply"), button[data-automation-id="adventureButton"]'
            )
            .first();
          if (!(await applyBtn.isVisible({ timeout: 8_000 }).catch(() => false))) {
            ctx.onStep?.({ stage, ok: false, note: 'Apply button not found' });
            return { ok: false, stage, submitted: false, url: postingUrl, note: 'apply-not-found' };
          }
          await applyBtn.click();
          ctx.onProgress?.(0.45, stage);

          stage = 'tenant-auth';
          // Workday almost always asks the user to create an account or sign in.
          // We surface this and stop — the user can complete it in the visible window.
          const signIn = page.locator('button:has-text("Sign In"), a:has-text("Sign In")').first();
          const create = page
            .locator('button:has-text("Create Account"), a:has-text("Create Account")')
            .first();
          const visibleAuth =
            (await signIn.isVisible({ timeout: 4_000 }).catch(() => false)) ||
            (await create.isVisible({ timeout: 4_000 }).catch(() => false));
          if (visibleAuth) {
            ctx.onStep?.({
              stage,
              ok: true,
              note: 'Workday requires tenant signup; complete in browser',
            });
            ctx.onProgress?.(0.7, stage);
          }

          stage = 'review-pending';
          ctx.onStep?.({ stage, ok: true, note: 'Workday submission requires manual review' });
          return {
            ok: true,
            stage,
            submitted: false,
            url: postingUrl,
            note: 'Workday flows are per-tenant; submission disabled in this build.',
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
}
