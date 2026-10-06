/**
 * DOM builders for the job-tools controls: the static skeleton, the "Check
 * fit" score card and the copy-field fallback rows. `textContent` only — no
 * `innerHTML` with page/desktop-derived text. All behaviour (listeners, state,
 * the trust gate) stays in `job-tools.ts`.
 */

import { el } from '../lib/dom';
import {
  IMPORT_LABEL_DEFAULT,
  JOB_TOOLS_GATED_LINE,
  type MatchLiveView,
  type ProfileFallbackField,
  scoreBand,
} from './responses';

/** Create `tag` and assign `props` (id, className, title, hidden, …) in one go. */
const make = <K extends keyof HTMLElementTagNameMap>(
  tag: K,
  props: Partial<HTMLElementTagNameMap[K]>,
  attrs: Record<string, string> = {}
): HTMLElementTagNameMap[K] => {
  const node = Object.assign(document.createElement(tag), props);
  for (const [name, value] of Object.entries(attrs)) node.setAttribute(name, value);
  return node;
};

const buttonOf = (
  id: string,
  className: string,
  textContent: string,
  props: Partial<HTMLButtonElement> = {}
): HTMLButtonElement => make('button', { id, type: 'button', className, textContent, ...props });

const STATUS_ATTRS = { role: 'status', 'aria-live': 'polite' };

/** The static controls, wired up by `mountJobTools`. */
export function buildJobToolsDom(hideSaveAnswers: boolean | undefined) {
  const gatedMsg = make(
    'p',
    { id: 'job-tools-gated', className: 'msg msg--muted', textContent: JOB_TOOLS_GATED_LINE },
    STATUS_ATTRS
  );
  const activeWrap = make('div', { id: 'job-tools-active' });

  const btnImport = buttonOf('btn-import', 'btn btn--primary', IMPORT_LABEL_DEFAULT);
  const btnCheckFit = buttonOf('btn-check-fit', 'btn btn--quiet', 'Check fit', {
    title: "Score your resume against this page's job posting",
  });
  const matchResult = make('div', { id: 'match-result', className: 'match-result', hidden: true });
  // "Stamp this results page" (PR3 §B.4) — hidden until the preference read
  // resolves it on (re-checked in `checkPage`, never cached at mount).
  const btnStampResults = buttonOf(
    'btn-stamp-results',
    'btn btn--quiet',
    'Stamp this results page',
    {
      title: 'Mark each visible job card on this results page saved or applied',
      hidden: true,
    }
  );
  const chkApplied = make('input', { id: 'chk-applied', type: 'checkbox' });
  const chkLabel = make('label', { className: 'check' });
  chkLabel.append(chkApplied, el('span', undefined, 'I already applied to this job'));
  const jobGroup = make('section', { className: 'group' }, { 'aria-label': 'Job' });
  jobGroup.append(btnImport, btnCheckFit, btnStampResults, matchResult, chkLabel);

  const btnFill = buttonOf('btn-fill', 'btn btn--primary', 'Fill this form', {
    title:
      "Fill this page's form with your saved contact details (opt-in, review before submitting)",
  });
  const btnSaveAnswers = buttonOf(
    'btn-save-answers',
    'btn btn--quiet',
    'Save my answers from this page',
    {
      title: "Save the answers you typed on this page's application form",
      hidden: Boolean(hideSaveAnswers),
    }
  );
  const formGroup = make(
    'section',
    { id: 'group-form', className: 'group group--divided' },
    { 'aria-label': 'Form' }
  );
  formGroup.append(btnFill, btnSaveAnswers);

  const msgEl = make('p', { id: 'job-tools-msg', className: 'msg' }, STATUS_ATTRS);

  // Copy-field fallback (decision 8) — shown only after a `fill` comes back
  // `filledNothing`. Never populated from storage — rebuilt fresh from a
  // `profileGet` reply each time it opens.
  const profileFallback = make(
    'section',
    { id: 'job-tools-profile-fallback', className: 'group group--divided', hidden: true },
    { 'aria-label': 'Your profile' }
  );

  activeWrap.append(jobGroup, formGroup, msgEl, profileFallback);
  return {
    gatedMsg,
    activeWrap,
    formGroup,
    btnImport,
    btnCheckFit,
    btnStampResults,
    btnFill,
    btnSaveAnswers,
    chkApplied,
    matchResult,
    msgEl,
    profileFallback,
  };
}

/** Build the "Check fit" score card — red-pen score circle, band, an
 *  expandable "why?" (missing-keyword chips + which résumé was used). */
export function buildMatchResultCard(view: MatchLiveView): HTMLElement {
  const card = el('div', 'fit-card');
  const band = view.score === null ? null : scoreBand(view.score);
  const head = el('div', 'fit-head');
  const headCopy = el('div', 'fit-head-copy');
  headCopy.append(
    el('p', 'match-result__score', band ? `${view.score}% fit · ${band}` : `${view.score}% fit`)
  );
  head.append(el('span', 'score-circle', `${view.score}%`), headCopy);
  card.append(head);

  const meta = [view.scoreLabel, view.resumeName ? `against “${view.resumeName}”` : null]
    .filter(Boolean)
    .join(' — ');
  if (meta) card.append(el('p', 'match-result__meta', meta));

  if (view.gaps.length > 0 || view.salary) {
    // Collapsed by default — open, the popup's connected+Check-fit view
    // overflows the 360×520 no-scroll budget (PR0 §2); the gap chips are one
    // tap away behind "why?".
    const why = el('details', 'why-toggle');
    why.append(el('summary', 'link', 'why?'));
    if (view.gaps.length > 0) {
      const gapsWrap = el('div', 'match-result__gaps');
      gapsWrap.append(...view.gaps.map((gap) => el('span', 'match-result__gap', gap)));
      why.append(gapsWrap);
    }
    if (view.salary) {
      // Two verbatim facts side by side, never a verdict (design decision 5)
      // — same wording the on-page fit badge uses (`lib/fit-badge.ts`).
      const bits = [`Posting says ${view.salary.posting}`];
      if (view.salary.expectation) bits.push(`You want ${view.salary.expectation}`);
      why.append(el('p', 'match-result__meta', bits.join(' · ')));
    }
    if (view.resumeName) {
      why.append(el('p', 'match-result__meta', `Résumé used: ${view.resumeName}`));
    }
    card.append(why);
  }
  return card;
}

/** Fill `section` with the profile fields (each with a Copy button), or hide
 *  it when there is nothing to show. */
export function renderProfileFallbackRows(
  section: HTMLElement,
  fields: ProfileFallbackField[],
  copy: (text: string) => Promise<boolean>
): void {
  section.replaceChildren();
  section.hidden = fields.length === 0;
  if (fields.length === 0) return;
  section.append(el('p', 'field-label', "Nothing matched — here's your profile"));
  for (const field of fields) {
    const row = el('div', 'profile-fallback-row');
    const copyBtn = el('button', 'btn btn--small btn--quiet', 'Copy');
    copyBtn.type = 'button';
    copyBtn.addEventListener('click', () => void copy(field.value));
    row.append(
      el('span', 'profile-fallback-row__value', `${field.label}: ${field.value}`),
      copyBtn
    );
    section.append(row);
  }
}
