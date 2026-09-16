/**
 * The on-page fit badge (PR3 §B.3, R4 of the redesign record) — rendered
 * after a successful Check-fit gesture, ONLY when `getShowFitBadge()` is
 * true (the caller's job, not this module's — this is pure DOM). Fixed
 * bottom-right pill (score + saved/applied chip + dismiss); clicking the
 * pill expands it to a mini card (missing keywords, the salary facts line
 * when present, one action "Open the panel"). No form actions, ever.
 *
 * Injected via `fit-badge.ts` (compiled to `fit-badge.js`, a classic
 * script), mirroring `fill.ts`'s two-step register-then-invoke pattern: the
 * match result is handed in transiently as ONE plain-object `executeScript`
 * arg (JSON-safe — the PR2 lesson: never a typed array or class instance).
 *
 * Pure DOM — no extension APIs beyond the one `runtime.sendMessage` the
 * "Open the panel" action posts — so the render logic itself is
 * unit-testable against a jsdom document.
 */

import { currentNotebookPalette, type NotebookPalette } from './notebook-palette';

/** Isolated-world global key `fit-badge.ts` exposes the renderer under. MUST
 *  match the literal duplicated in `background.ts` (same discipline as
 *  `AUTOFILL_GLOBAL`). */
export const FIT_BADGE_GLOBAL = '__ajhRenderFitBadge';

/** Internal message kind the badge's "Open the panel" button posts — MUST
 *  match the literal duplicated in `background.ts`. */
export const OPEN_PANEL_MSG = 'ajhOpenPanelFromBadge';

/** DOM id of the injected badge (also used to clear a prior pass — a second
 *  Check-fit on the same page replaces the first, never stacks). */
const BADGE_ID = 'ajh-fit-badge';

/** The plain-object shape the background hands to the injected renderer —
 *  every field a JSON-safe primitive/array/plain-object (PR2 lesson). */
export interface FitBadgeView {
  score: number;
  band: 'strong match' | 'partial match' | 'low match';
  /** Human-readable score-source qualifier (e.g. "keyword coverage") — the
   *  panel/popup card always shows one (`job-tools.ts`'s `SCORE_SOURCE_LABEL`);
   *  this on-page badge must too, not bury it one tap deeper than the panel
   *  does. */
  scoreLabel: string;
  /** Top missing keywords (already capped by the desktop; the badge shows
   *  at most a further-trimmed handful — see {@link MAX_BADGE_GAPS}). */
  gaps: string[];
  /** `null` when the job is neither saved nor applied for this url. */
  applied: 'saved' | 'applied' | null;
  salary?: { posting: string; expectation?: string };
}

/** The mini card shows only the top few gaps — the "why?" details in the
 *  popup/panel card already shows the full (≤8) list; the on-page badge is
 *  a glance, not a second full report. */
const MAX_BADGE_GAPS = 5;

function bandLabel(band: FitBadgeView['band']): string {
  return band;
}

/**
 * Render (or replace) the fit badge on `doc`. Idempotent: a second call
 * removes any prior badge first, so a repeat Check-fit never stacks two.
 *
 * Returns the closed `ShadowRoot` — only this caller's own closure keeps
 * that reference (used by tests to inspect content); `root.shadowRoot` is
 * `null` to everyone else, including the page's own main-world scripts.
 */
export function renderFitBadge(
  doc: Document,
  palette: NotebookPalette,
  view: FitBadgeView
): ShadowRoot {
  doc.getElementById(BADGE_ID)?.remove();

  const root = doc.createElement('div');
  root.id = BADGE_ID;
  // Closed shadow root: `root.shadowRoot` is null from outside, so the
  // page's own scripts (ads/trackers/a compromised board) can't read the
  // salary/gap text off the shared DOM even though `root` itself lives in
  // `doc.body` — see the fit-badge review finding this fixes.
  const shadow = root.attachShadow({ mode: 'closed' });
  root.style.cssText = [
    'position:fixed',
    'z-index:2147483647',
    'right:16px',
    'bottom:16px',
    'max-width:300px',
    `font:13px/1.4 system-ui,-apple-system,'Segoe UI',Roboto,sans-serif`,
    `background:${palette.card}`,
    `color:${palette.ink}`,
    `border:2px solid ${palette.ink}`,
    'border-radius:14px',
    `box-shadow:3px 4px 0 ${palette.shadow}`,
    'padding:10px 12px',
  ].join(';');

  // ── the pill (always visible) ─────────────────────────────────────────
  const pill = doc.createElement('button');
  pill.type = 'button';
  // The disclosure button controls `card`'s visibility — `aria-expanded` and
  // the trailing clause of the label both track that state (set here for the
  // initial collapsed render, updated together with `card.hidden` in the
  // click handler below), so assistive tech is never told "Expand for
  // details" once the details are already showing.
  const pillAriaLabelBase = `AI Job Hunter fit: ${view.score}% — ${bandLabel(view.band)}, ${view.scoreLabel}.`;
  pill.setAttribute('aria-label', `${pillAriaLabelBase} Expand for details.`);
  pill.setAttribute('aria-expanded', 'false');
  pill.style.cssText = [
    'display:flex',
    'align-items:center',
    'gap:8px',
    'width:100%',
    'border:0',
    'background:transparent',
    `color:${palette.ink}`,
    'cursor:pointer',
    'font:inherit',
    'padding:0',
    'text-align:left',
  ].join(';');

  const circle = doc.createElement('span');
  circle.textContent = `${view.score}%`;
  circle.style.cssText = [
    'display:inline-flex',
    'align-items:center',
    'justify-content:center',
    'width:34px',
    'height:34px',
    'border-radius:50%',
    `border:2px solid ${palette.red}`,
    `color:${palette.redInk}`,
    'font-weight:700',
    'flex-shrink:0',
  ].join(';');
  pill.append(circle);

  const label = doc.createElement('span');
  label.style.cssText = 'flex:1;min-width:0';
  const labelBits = [bandLabel(view.band)];
  if (view.applied) labelBits.push(view.applied === 'applied' ? 'Applied' : 'Saved');
  label.textContent = labelBits.join(' · ');
  pill.append(label);

  shadow.append(pill);

  // ── the dismiss control (always visible, next to the pill) ────────────
  const dismiss = doc.createElement('button');
  dismiss.type = 'button';
  dismiss.textContent = '×';
  dismiss.setAttribute('aria-label', 'Dismiss the fit badge');
  dismiss.style.cssText = [
    'position:absolute',
    'top:6px',
    'right:8px',
    'border:0',
    'background:transparent',
    `color:${palette.inkSoft}`,
    'cursor:pointer',
    'font:16px/1 system-ui,sans-serif',
    'padding:2px 4px',
  ].join(';');
  dismiss.addEventListener('click', (e) => {
    e.stopPropagation();
    root.remove();
  });
  shadow.append(dismiss);

  // ── the mini card (collapsed by default, content built lazily on first
  // expand — defense in depth alongside the closed shadow root above: the
  // gaps/salary text doesn't exist in the DOM at all until the user clicks).
  const card = doc.createElement('div');
  card.hidden = true;
  card.style.cssText = 'margin-top:10px;padding-top:10px';
  card.style.borderTop = `1px solid ${palette.inkSoft}`;
  shadow.append(card);

  let cardPopulated = false;
  function populateCard(): void {
    if (cardPopulated) return;
    cardPopulated = true;

    // Score-source qualifier — always shown, mirroring the panel/popup
    // card's always-visible meta line (`job-tools.ts::buildMatchResultCard`),
    // never buried behind the "why?" gaps toggle.
    const scoreMeta = doc.createElement('p');
    scoreMeta.textContent = view.scoreLabel;
    scoreMeta.style.cssText = `margin:0 0 8px;font-size:12px;color:${palette.inkSoft}`;
    card.append(scoreMeta);

    if (view.gaps.length > 0) {
      const gapsHeading = doc.createElement('p');
      gapsHeading.textContent = 'Missing keywords';
      gapsHeading.style.cssText = `margin:0 0 4px;color:${palette.inkSoft};font-size:12px`;
      card.append(gapsHeading);

      const chips = doc.createElement('div');
      chips.style.cssText = 'display:flex;flex-wrap:wrap;gap:4px;margin-bottom:8px';
      for (const gap of view.gaps.slice(0, MAX_BADGE_GAPS)) {
        const chip = doc.createElement('span');
        chip.textContent = gap;
        chip.style.cssText = [
          'display:inline-block',
          'padding:2px 8px',
          'border-radius:999px',
          `border:1px solid ${palette.warn}`,
          `color:${palette.warn}`,
          'font-size:12px',
        ].join(';');
        chips.append(chip);
      }
      card.append(chips);
    }

    if (view.salary) {
      const salaryLine = doc.createElement('p');
      salaryLine.style.cssText = `margin:0 0 8px;font-size:12px;color:${palette.inkSoft}`;
      // Two verbatim facts, side by side — never a verdict (design decision 5).
      const bits = [`Posting says ${view.salary.posting}`];
      if (view.salary.expectation) bits.push(`You want ${view.salary.expectation}`);
      salaryLine.textContent = bits.join(' · ');
      card.append(salaryLine);
    }

    const openPanel = doc.createElement('button');
    openPanel.type = 'button';
    openPanel.textContent = 'Open the panel';
    openPanel.style.cssText = [
      'border:0',
      `background:${palette.red}`,
      'color:#fff',
      'border-radius:8px',
      'padding:6px 10px',
      'cursor:pointer',
      'font:inherit',
      'font-weight:600',
    ].join(';');
    openPanel.addEventListener('click', () => {
      try {
        const w = doc.defaultView as
          (Window & { chrome?: { runtime?: { sendMessage?: (m: unknown) => void } } }) | null;
        w?.chrome?.runtime?.sendMessage?.({ kind: OPEN_PANEL_MSG });
      } catch {
        // Best-effort only — a page without the runtime bridge simply gets no-op.
      }
    });
    card.append(openPanel);
  }

  pill.addEventListener('click', () => {
    populateCard();
    card.hidden = !card.hidden;
    const expanded = !card.hidden;
    pill.setAttribute('aria-expanded', String(expanded));
    pill.setAttribute(
      'aria-label',
      `${pillAriaLabelBase} ${expanded ? 'Collapse the details.' : 'Expand for details.'}`
    );
  });

  (doc.body ?? doc.documentElement).appendChild(root);
  return shadow;
}

/**
 * The injected entry-point: pick the palette from the page's own preference
 * and render. Kept side-effect-first so `executeScript` gets a serializable
 * (here, `void`/`undefined`) completion value — this entry communicates by
 * installing a global, not a completion value (see `build-output.test.ts`'s
 * `GLOBAL_INSTALLING_ENTRIES`).
 */
export function runRenderFitBadge(view: FitBadgeView): void {
  const palette = currentNotebookPalette(window);
  renderFitBadge(document, palette, view);
}
