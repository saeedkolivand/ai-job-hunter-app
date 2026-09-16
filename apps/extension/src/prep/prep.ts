/**
 * Prep tab (PR4 §B.2–B.3) — mounted by the side panel only (the popup stays
 * the compact launcher, R1 of the redesign record). Reads this job's existing
 * generations (company brief, interview questions, salary answer) through the
 * curated `prep` read-tier resource (PR1, zero cost), and drives two
 * on-demand drafts (company brief, salary answer) through the SAME
 * `answer.assist` streaming/cancel machinery `answer-tools.ts` uses for a row
 * — correlated by `topic` instead of `rowId` (see `lib/answer-state.ts`'s
 * `AnswerStream.topic` doc for why the two never collide). Interview-question
 * generation stays in-app (decision 6 of the redesign record) — this tab only
 * ever DISPLAYS whatever the app has already generated.
 *
 * Every block is a native `<details>` — collapsed/scannable, and toggled ONLY
 * by a click (design decision 6: "click-triggered only, never hover"), which
 * `<details>` already guarantees without any bespoke JS.
 *
 * `mountX(host, deps)` — same pattern as `documents.ts`/`job-tools.ts`.
 */

import { browser } from '@wxt-dev/browser';

import type { AnswerState } from '../lib/answer-state';
import type { PopupRequest, PopupResponse } from '../lib/messages';

// ── pure data shaping (exported for unit tests) ─────────────────────────────

export interface InterviewQuestionSummary {
  question: string;
  why?: string;
  audience?: string;
}

export interface PrepResourceData {
  hasCompanyBrief: boolean;
  companyBrief: string | null;
  interviewQuestions: InterviewQuestionSummary[];
  salaryAnswer: string | null;
}

const EMPTY_PREP_DATA: PrepResourceData = {
  hasCompanyBrief: false,
  companyBrief: null,
  interviewQuestions: [],
  salaryAnswer: null,
};

/** Hand-written guard for the `prep` resource's `data` shape (PR4 §A.1) — the
 *  extension stays zod-free everywhere, same discipline as `documents.ts`'s
 *  `parseDocumentsResourceData`. Ignores fields this tab doesn't render
 *  (`updatedAt`, `truncated`) rather than validating every one of them. */
export function parsePrepResourceData(data: unknown): PrepResourceData {
  if (typeof data !== 'object' || data === null) return EMPTY_PREP_DATA;
  const o = data as Record<string, unknown>;
  const gen = o.generation;
  if (typeof gen !== 'object' || gen === null) return EMPTY_PREP_DATA;
  const g = gen as Record<string, unknown>;

  const interviewQuestions = (Array.isArray(g.interviewQuestions) ? g.interviewQuestions : [])
    .filter((q): q is Record<string, unknown> => typeof q === 'object' && q !== null)
    .filter((q) => typeof q.question === 'string')
    .map((q) => ({
      question: q.question as string,
      why: typeof q.why === 'string' ? q.why : undefined,
      audience: typeof q.audience === 'string' ? q.audience : undefined,
    }));

  return {
    hasCompanyBrief: g.hasCompanyBrief === true,
    companyBrief: typeof g.companyBrief === 'string' ? g.companyBrief : null,
    interviewQuestions,
    salaryAnswer: typeof g.salaryAnswer === 'string' ? g.salaryAnswer : null,
  };
}

/** Whether the tab has anything at all to show — drives the "nothing yet"
 *  empty state vs. the populated one. Pure. */
export function prepHasContent(data: PrepResourceData): boolean {
  return data.hasCompanyBrief || data.interviewQuestions.length > 0 || data.salaryAnswer !== null;
}

// ── the view ─────────────────────────────────────────────────────────────

export type PrepTopic = 'company-brief' | 'salary-answer';

export interface PrepDeps {
  send: (req: PopupRequest) => Promise<PopupResponse>;
  copy: (text: string) => Promise<boolean>;
}

export interface PrepView {
  /** Feed the latest per-tab `AnswerState` — used ONLY to watch the shared
   *  `answer.assist` stream for a topic-tagged draft in flight. Never a
   *  trust gate of its own — see `documents.ts`'s doc for the same shape. */
  render: (state: AnswerState | null) => void;
  /** Re-fetch this tab's prep data + the AI-assist opt-in. Call only while
   *  the followed tab is trusted (mirrors `documents.ts`'s `refresh`). */
  refresh: () => void;
  /** Clear data + any in-flight draft — call on an untrusted tab / a tab
   *  switch (mirrors `documents.ts`'s `reset`). */
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

const button = (className: string, label: string): HTMLButtonElement => {
  const b = el('button', className, label);
  b.type = 'button';
  return b;
};

const TOPIC_LABEL: Record<PrepTopic, string> = {
  'company-brief': 'Company brief',
  'salary-answer': 'Salary answer',
};

export function mountPrep(host: HTMLElement, deps: PrepDeps): PrepView {
  let data: PrepResourceData = EMPTY_PREP_DATA;
  let aiAssistEnabled: boolean | null = null; // null = unknown until settings.get answers
  let state: AnswerState | null = null;
  let generation = 0;
  let lastUrl = '';
  let statusText = '';
  let statusTone: 'ok' | 'err' | 'muted' = 'muted';
  /** Draft text finalized by a completed on-demand request, kept SEPARATE
   *  from the live `state.stream` mirror so the copy-ready text survives a
   *  later, unrelated stream (e.g. an Answer-tools row draft) clobbering
   *  `state.stream`. */
  const finishedDrafts: Partial<Record<PrepTopic, string>> = {};
  let pendingTopic: PrepTopic | null = null;

  const setStatus = (text: string, tone: 'ok' | 'err' | 'muted'): void => {
    statusText = text;
    statusTone = tone;
  };

  async function openPrepLink(): Promise<void> {
    try {
      await browser.tabs.create({ url: `ajh://prep?url=${encodeURIComponent(lastUrl)}` });
    } catch {
      // No-op: the deep link is best-effort — same discipline as
      // documents.ts's own "Generate in the app" link.
    }
  }

  function openAiAssistSettings(): void {
    void browser.runtime.openOptionsPage();
  }

  function renderCopyableSection(title: string, text: string): HTMLElement {
    const details = document.createElement('details');
    details.className = 'prep-section';
    const summary = document.createElement('summary');
    summary.textContent = title;
    details.append(summary);
    const body = el('p', 'prep-section__text', text);
    details.append(body);
    const copyBtn = button('btn btn--small btn--quiet', 'Copy');
    copyBtn.addEventListener('click', () => void doCopy(text));
    details.append(copyBtn);
    return details;
  }

  function renderDraftButton(topic: PrepTopic): HTMLElement {
    const wrap = el('div', 'prep-draft');
    if (aiAssistEnabled === false) {
      wrap.append(el('p', 'msg msg--muted', 'AI-answer-assist is off.'));
      const link = button('btn btn--small btn--quiet', 'Turn on in Settings');
      link.addEventListener('click', openAiAssistSettings);
      wrap.append(link);
      return wrap;
    }
    const streaming = pendingTopic === topic;
    const draft = streaming ? (state?.stream?.topic === topic ? state.stream.text : '') : undefined;
    if (streaming) {
      wrap.append(el('p', 'prep-section__text', draft || 'Drafting…'));
      const cancelBtn = button('btn btn--small btn--quiet', 'Cancel');
      cancelBtn.addEventListener('click', () => void doCancel());
      wrap.append(cancelBtn);
      return wrap;
    }
    const draftBtn = button('btn btn--small', `Draft ${TOPIC_LABEL[topic].toLowerCase()}`);
    // Disabled while unknown, OR while the OTHER topic is streaming — a click
    // here would silently supersede that in-flight draft with no visible
    // explanation (the two share one `answer.assist` slot; see `doDraft`).
    draftBtn.disabled = aiAssistEnabled === null || pendingTopic !== null;
    draftBtn.addEventListener('click', () => void doDraft(topic));
    wrap.append(draftBtn);
    return wrap;
  }

  function render(): void {
    host.replaceChildren();

    const hasContent = prepHasContent(data);
    if (!hasContent && !finishedDrafts['company-brief'] && !finishedDrafts['salary-answer']) {
      host.append(el('p', 'msg msg--muted', statusText || 'Loading…'));
      if (statusText && statusTone === 'muted' && lastUrl) {
        const link = button('btn btn--quiet', 'Prepare in the app');
        link.addEventListener('click', () => void openPrepLink());
        host.append(link);
      }
      // The two on-demand buttons are still offered on an otherwise-empty job
      // — a fresh job has no generation yet, but the drafts don't need one.
      host.append(renderDraftButton('company-brief'));
      host.append(renderDraftButton('salary-answer'));
      return;
    }

    const brief = finishedDrafts['company-brief'] ?? data.companyBrief;
    if (brief) {
      host.append(renderCopyableSection('Company brief', brief));
    } else {
      host.append(el('p', 'field-label', 'Company brief'));
      host.append(renderDraftButton('company-brief'));
    }

    if (data.interviewQuestions.length > 0) {
      const details = document.createElement('details');
      details.className = 'prep-section';
      const summary = document.createElement('summary');
      summary.textContent = `Interview questions (${data.interviewQuestions.length})`;
      details.append(summary);
      for (const q of data.interviewQuestions) {
        const item = el('div', 'prep-question');
        item.append(el('p', 'prep-question__text', q.question));
        if (q.why) item.append(el('p', 'msg msg--muted', q.why));
        const copyBtn = button('btn btn--small btn--quiet', 'Copy');
        copyBtn.addEventListener('click', () => void doCopy(q.question));
        item.append(copyBtn);
        details.append(item);
      }
      host.append(details);
    }

    const salary = finishedDrafts['salary-answer'] ?? data.salaryAnswer;
    if (salary) {
      host.append(renderCopyableSection('Salary answer', salary));
    } else {
      host.append(el('p', 'field-label', 'Salary answer'));
      host.append(renderDraftButton('salary-answer'));
    }

    if (statusText) {
      host.append(el('p', `msg msg--${statusTone === 'muted' ? 'muted' : statusTone}`, statusText));
    }
  }

  async function doCopy(text: string): Promise<void> {
    const ok = await deps.copy(text);
    setStatus(ok ? 'Copied.' : 'Could not copy — try again.', ok ? 'ok' : 'err');
    render();
  }

  async function doCancel(): Promise<void> {
    pendingTopic = null;
    render();
    await deps.send({ kind: 'assistCancel' }).catch(() => undefined);
  }

  async function doDraft(topic: PrepTopic): Promise<void> {
    if (aiAssistEnabled !== true) return;
    pendingTopic = topic;
    setStatus('', 'muted');
    render();
    try {
      const res = await deps.send({
        kind: 'answerAssist',
        question: TOPIC_LABEL[topic],
        searchWeb: false,
        topic,
      });
      if (pendingTopic !== topic) return; // superseded by a cancel/newer draft
      pendingTopic = null;
      if (!res.ok) {
        setStatus(res.error, 'err');
        render();
        return;
      }
      if (res.kind !== 'answerAssist') {
        setStatus('Unexpected response — please retry.', 'err');
        render();
        return;
      }
      if (!res.result.ok) {
        setStatus(res.result.error, 'err');
        render();
        return;
      }
      finishedDrafts[topic] = res.result.draft;
      setStatus('', 'muted');
      render();
    } catch (err) {
      if (pendingTopic !== topic) return;
      pendingTopic = null;
      setStatus(err instanceof Error ? err.message : String(err), 'err');
      render();
    }
  }

  async function refresh(): Promise<void> {
    generation += 1;
    const myGeneration = generation;
    setStatus('', 'muted');
    render();
    try {
      const [prepRes, settingsRes] = await Promise.all([
        deps.send({ kind: 'prepGet' }),
        deps.send({ kind: 'settingsGet' }),
      ]);
      if (myGeneration !== generation) return;
      if (settingsRes.ok && settingsRes.kind === 'settingsGet' && settingsRes.result.ok) {
        aiAssistEnabled = settingsRes.result.settings.aiAssist;
      }
      if (!prepRes.ok) {
        data = EMPTY_PREP_DATA;
        setStatus(prepRes.error, 'err');
        render();
        return;
      }
      if (prepRes.kind !== 'prepGet') return;
      lastUrl = prepRes.url;
      if (!prepRes.result.ok) {
        data = EMPTY_PREP_DATA;
        setStatus(prepRes.result.error, 'err');
        render();
        return;
      }
      data = parsePrepResourceData(prepRes.result.data);
      setStatus(prepHasContent(data) ? '' : 'Nothing yet for this job.', 'muted');
      render();
    } catch (err) {
      if (myGeneration !== generation) return;
      data = EMPTY_PREP_DATA;
      setStatus(err instanceof Error ? err.message : String(err), 'err');
      render();
    }
  }

  function reset(): void {
    generation += 1;
    data = EMPTY_PREP_DATA;
    lastUrl = '';
    pendingTopic = null;
    delete finishedDrafts['company-brief'];
    delete finishedDrafts['salary-answer'];
    setStatus('', 'muted');
    render();
  }

  render();

  return {
    render: (next) => {
      state = next;
      if (pendingTopic !== null) render();
    },
    refresh: () => void refresh(),
    reset,
  };
}
