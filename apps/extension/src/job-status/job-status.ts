/**
 * The page-context card + read-only application-stage strip, mounted by BOTH
 * the popup (compact launcher) and the side panel's Job tab (PR0 §2/§3). Built
 * on the SAME `appliedCheck` verb `popup.ts`'s own auto-check already used —
 * no new bridge verb, no new popup↔background message kind.
 *
 * What this card can show is bounded by what `appliedCheck` actually carries
 * (`title`/`status`/`appliedAt` — see `lib/messages.ts`'s
 * `ExtensionAppliedCheckResult`): company/location are NOT in that reply, so
 * unlike the design mockup this card shows title + status only until a later
 * PR adds a source for them. The stage strip only ever marks Saved/Applied
 * from the extension's own writes — Interview/Offer render as reached when
 * the desktop's own status says so, matching the caption below.
 */

import type { PopupRequest, PopupResponse } from '../lib/messages';

export interface StageSpec {
  key: 'saved' | 'applied' | 'interviewing' | 'offer';
  label: string;
}

export const STAGES: readonly StageSpec[] = [
  { key: 'saved', label: 'Saved' },
  { key: 'applied', label: 'Applied' },
  { key: 'interviewing', label: 'Interview' },
  { key: 'offer', label: 'Offer' },
];

/** Index of the current stage for `status`, or `-1` when the status is
 *  unknown/unmapped (nothing highlighted). Pure. */
export function stageIndex(status: string | undefined): number {
  if (!status) return -1;
  return STAGES.findIndex((s) => s.key === status);
}

/** The one-line caption under the stage strip. */
export const STAGE_CAPTION = 'The extension only marks Applied; change stages in the app.';

/** What the page-context card should render, or `null` when there is nothing
 *  found for the active tab's url (the caller then shows no card at all). */
export interface JobStatusView {
  title: string | null;
  chipText: string;
  currentStageIndex: number;
}

function formatShortDate(epochMs: number): string {
  const date = new Date(epochMs);
  const opts: Intl.DateTimeFormatOptions = { month: 'short', day: 'numeric' };
  if (date.getFullYear() !== new Date().getFullYear()) opts.year = 'numeric';
  return date.toLocaleDateString(undefined, opts);
}

/**
 * Given an `appliedCheck` response, the view to render, or `null` when
 * nothing was found (any error folds into `null` too — same "silent
 * best-effort" discipline as `popup.ts`'s `resolveAppliedStatusLine`).
 *
 * Pure: no DOM access, no side effects.
 */
export function resolveJobStatusView(res: PopupResponse): JobStatusView | null {
  if (!res.ok || res.kind !== 'appliedCheck') return null;
  const { result } = res;
  if (result.error || !result.found) return null;

  const status = result.status ?? 'saved';
  const label = STAGES.find((s) => s.key === status)?.label ?? 'Saved';
  const when = typeof result.appliedAt === 'number' ? formatShortDate(result.appliedAt) : null;
  const chipText = when ? `${label} ${when}` : label;

  return {
    title: result.title?.trim() || null,
    chipText,
    currentStageIndex: stageIndex(status),
  };
}

export interface JobStatusDeps {
  send: (req: PopupRequest) => Promise<PopupResponse>;
}

export interface JobStatusHandle {
  /** Run `appliedCheck` for the active tab and render the result (or clear
   *  the card when nothing was found). Fire-and-forget, mirrors the existing
   *  auto-check's never-throws discipline. */
  refresh: () => Promise<void>;
  /** Reset to the empty/no-page state (caller: on leaving `connected`). */
  reset: () => void;
}

const el = <K extends keyof HTMLElementTagNameMap>(
  tag: K,
  className?: string,
  text?: string
): HTMLElementTagNameMap[K] => {
  const node = document.createElement(tag);
  if (className) node.className = className;
  if (text !== undefined) node.textContent = text;
  return node;
};

/** Mount the page-context card + stage strip into `host`. Hidden while there
 *  is nothing found for the active tab. */
export function mountJobStatus(host: HTMLElement, deps: JobStatusDeps): JobStatusHandle {
  const card = el('div', 'card');
  card.hidden = true;
  host.append(card);

  function renderStrip(currentStageIndex: number): HTMLElement {
    const strip = el('div', 'stage-strip');
    strip.setAttribute('aria-label', 'Application stage');
    STAGES.forEach((stage, i) => {
      if (i > 0) strip.append(el('span', 'stage-arrow', '→'));
      const pill = el('span', 'stage', stage.label);
      if (i === currentStageIndex) pill.classList.add('current');
      strip.append(pill);
    });
    return strip;
  }

  function render(view: JobStatusView): void {
    card.replaceChildren();
    if (view.title) card.append(el('p', 'job-title', view.title));
    const chipRow = el('div', 'chip-row');
    chipRow.append(el('span', 'tag tag--ok', view.chipText));
    card.append(chipRow);
    card.append(renderStrip(view.currentStageIndex));
    card.append(el('p', 'stage-caption', STAGE_CAPTION));
    card.hidden = false;
  }

  function reset(): void {
    card.hidden = true;
    card.replaceChildren();
  }

  async function refresh(): Promise<void> {
    try {
      const res = await deps.send({ kind: 'appliedCheck' });
      const view = resolveJobStatusView(res);
      if (view) render(view);
      else reset();
    } catch {
      reset();
    }
  }

  return { refresh, reset };
}
