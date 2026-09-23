/**
 * Documents tab (PR2 §C.2–C.5) — mounted by the side panel only (the popup
 * stays the compact launcher, R1 of the redesign record). Lists this job's
 * generation + saved base résumés (the curated `documents` read-tier
 * resource, PR1), lets the user pick a source/kind/template/format, and
 * offers: Attach résumé to this page, Paste cover letter…, Copy cover
 * letter, and (when the job has no generation) a "Generate in the app" deep
 * link.
 *
 * Every action here touches the active tab exactly like Fill does — reads
 * `activeTabUrl()`/injects a script — so `refresh()`/`reset()` are driven by
 * the SAME `isPageTrusted` gate `job-tools.ts` uses (its own doc explains
 * why a panel-button click alone never re-grants `activeTab`); the caller
 * (`sidepanel.ts`) calls them only when the followed tab is trusted, exactly
 * where it already calls `jobStatus.refresh()`/`refreshTrustLineJob()`. On an
 * untrusted tab the caller passes the shared {@link
 * ../job-tools/job-tools.ts#JOB_TOOLS_GATED_LINE} to `reset()` — the SAME one
 * line `job-tools.ts` renders under the Job controls, so both surfaces get a
 * single, shared "grant access" sentence instead of per-tab duplicates
 * (job-tools.ts's own doc explains why ONE shared line beats more than one).
 *
 * `mountX(host, deps)` — same pattern as `job-tools.ts`/`answer-tools.ts`.
 */

import { browser } from '@wxt-dev/browser';

import type { ExtensionDocumentSource } from '@ajh/shared/extension-protocol';
import { LETTER_LAYOUT_LABELS, TEMPLATE_LABELS } from '@ajh/shared/ipc';

import type { AnswerState } from '../lib/answer-state';
import type { PopupRequest, PopupResponse } from '../lib/messages';

// ── pure data shaping (exported for unit tests) ─────────────────────────────

/** One candidate document the picker can export from. */
export interface DocumentCandidate {
  source: ExtensionDocumentSource;
  label: string;
  /** Only a `generation` source can ever have a cover letter. */
  hasCoverLetter: boolean;
}

interface GenerationSummary {
  hasResume: boolean;
  hasCoverLetter: boolean;
  jobTitle?: string;
  company?: string;
}

interface DocumentSummary {
  id: string;
  name: string;
}

interface DocumentsResourceData {
  generation: GenerationSummary | null;
  documents: DocumentSummary[];
}

/** Hand-written guard for the `documents` resource's `data` shape (PR2
 *  §A.2) — the extension stays zod-free everywhere, same discipline as
 *  `bridge.ts`. Ignores fields this picker doesn't render (`targetLanguage`,
 *  `updatedAt`, `language`) rather than validating every one of them. */
export function parseDocumentsResourceData(data: unknown): DocumentsResourceData {
  const EMPTY: DocumentsResourceData = { generation: null, documents: [] };
  if (typeof data !== 'object' || data === null) return EMPTY;
  const o = data as Record<string, unknown>;

  let generation: GenerationSummary | null = null;
  if (typeof o.generation === 'object' && o.generation !== null) {
    const g = o.generation as Record<string, unknown>;
    if (typeof g.hasResume === 'boolean' && typeof g.hasCoverLetter === 'boolean') {
      generation = {
        hasResume: g.hasResume,
        hasCoverLetter: g.hasCoverLetter,
        jobTitle: typeof g.jobTitle === 'string' ? g.jobTitle : undefined,
        company: typeof g.company === 'string' ? g.company : undefined,
      };
    }
  }

  const documents = (Array.isArray(o.documents) ? o.documents : [])
    .filter((d): d is Record<string, unknown> => typeof d === 'object' && d !== null)
    .filter((d) => typeof d.id === 'string' && typeof d.name === 'string')
    .map((d) => ({ id: d.id as string, name: d.name as string }));

  return { generation, documents };
}

/**
 * Build the picker's candidate list from the resource data — the job's own
 * generation first (when it has a résumé to export at all), then the saved
 * base résumés, newest first (the resource itself orders them that way — see
 * the Rust doc). `url` is the active tab's url (background-resolved, echoed
 * back — see `PopupResponse.documentsList`'s doc).
 */
export function buildCandidates(data: DocumentsResourceData, url: string): DocumentCandidate[] {
  const candidates: DocumentCandidate[] = [];
  if (data.generation?.hasResume) {
    const title = data.generation.jobTitle?.trim();
    const company = data.generation.company?.trim();
    const label = title && company ? `${title} · ${company}` : (title ?? company ?? 'This job');
    candidates.push({
      source: { kind: 'generation', url },
      label,
      hasCoverLetter: data.generation.hasCoverLetter,
    });
  }
  for (const doc of data.documents) {
    candidates.push({
      source: { kind: 'document', id: doc.id },
      label: doc.name,
      hasCoverLetter: false,
    });
  }
  return candidates;
}

// ── the view ─────────────────────────────────────────────────────────────

export interface DocumentsDeps {
  send: (req: PopupRequest) => Promise<PopupResponse>;
  copy: (text: string) => Promise<boolean>;
  /** Asked BEFORE an Attach — reuses the panel's existing first-time Fill
   *  confirmation (R6), with this flow's own copy. Resolves `true` to
   *  proceed, `false` to cancel. */
  confirmAttach: (host: string | null) => Promise<boolean>;
  /** The active tab's hostname, for {@link confirmAttach} — mirrors
   *  `sidepanel.ts`'s own `currentOrigin`/`hostOf` snapshot pattern; kept a
   *  function (not a static string) so it always reads the CURRENT tab at
   *  click time. */
  currentHost: () => string | null;
  /** `sidepanel.ts`'s own `followGeneration` (PR review round 2) — bumped
   *  every time `follow()` re-targets the panel at a different tab. The
   *  cover-letter paste flow ({@link doPasteInto}) snapshots this alongside
   *  the fed `AnswerState.tabId` before its export wait and re-checks both
   *  right before sending `answerFill`/`answerReplace`, so a tab switch
   *  during that wait can never paste into a page the user did not pick. */
  getFollowGeneration: () => number;
  /** Called with the active tab's url every time `refresh()` resolves one —
   *  lets the caller (`sidepanel.ts`) drive the Job tab header's "Open in
   *  app" deep link from the SAME resolved url, without a second
   *  `documentsList` round trip. */
  onUrlResolved?: (url: string) => void;
}

export interface DocumentsView {
  /** Feed the latest per-tab `AnswerState` — used ONLY to build the
   *  cover-letter paste-target picker (rows with a live `.field`). Never a
   *  trust gate of its own — see this module's doc. */
  render: (state: AnswerState | null) => void;
  /** Re-fetch this tab's document candidates. Call only while the followed
   *  tab is trusted (mirrors `job-tools.ts`'s `checkPage`). */
  refresh: () => void;
  /** Clear candidates + any open picker — call on an untrusted tab / a tab
   *  switch (mirrors `job-status.ts`'s `reset`). `reason` renders in place of
   *  the loading/status line when there is nothing to show — the caller's
   *  shared {@link ../job-tools/job-tools.ts#JOB_TOOLS_GATED_LINE} for an
   *  untrusted page (#1225), '' (nothing) for a plain clear. */
  reset: (reason?: string) => void;
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

export function mountDocuments(host: HTMLElement, deps: DocumentsDeps): DocumentsView {
  let candidates: DocumentCandidate[] = [];
  let selectedIndex = 0;
  let kind: 'resume' | 'cover-letter' = 'resume';
  let templateId = 'classic';
  let letterLayoutId = 'classic';
  let format: 'pdf' | 'docx' = 'pdf';
  let state: AnswerState | null = null;
  let pastePickerOpen = false;
  let busy = false;
  let generation = 0;
  /** The active tab's url, echoed back by the last `documentsList` reply —
   *  needed for the "Generate in the app" deep link even when there are no
   *  candidates to pick from at all (decision 4 of the redesign record). */
  let lastUrl = '';

  let statusText = '';
  let statusTone: 'ok' | 'err' | 'muted' = 'muted';
  /** Whether a `refresh()` is genuinely in flight — TRUE only between a
   *  refresh's start and its terminal render, cleared in EVERY terminal
   *  branch (a resolved reply, a desktop refusal, a kind mismatch, a thrown
   *  error). The generation-guarded early `return`s (a newer refresh/reset
   *  has superseded this one) deliberately leave it untouched — the newer
   *  call owns `loading` now. Drives the empty-state's "Loading…" line, so
   *  the mount alone and a settled refresh can never show a phantom one
   *  (#1225). */
  let loading = false;
  const setStatus = (text: string, tone: 'ok' | 'err' | 'muted'): void => {
    statusText = text;
    statusTone = tone;
  };

  function selected(): DocumentCandidate | null {
    return candidates[selectedIndex] ?? null;
  }

  async function openGenerateLink(): Promise<void> {
    try {
      await browser.tabs.create({ url: `ajh://generate?url=${encodeURIComponent(lastUrl)}` });
    } catch {
      // No-op: the deep link is best-effort — same discipline as
      // connection-status.ts's own deep links.
    }
  }

  function render(): void {
    host.replaceChildren();

    if (candidates.length === 0) {
      if (loading) {
        // "Loading…" ONLY while a refresh is genuinely in flight — never at
        // mount, never after a settled refresh (its terminal branch clears
        // `loading`), never after a reset (#1225).
        host.append(el('p', 'msg msg--muted', 'Loading…'));
      } else if (statusText) {
        host.append(el('p', 'msg msg--muted', statusText));
        if (statusTone === 'muted' && lastUrl) {
          // A button, not an `<a href="ajh://…">` — mirrors the already-verified
          // deep-link trigger the rest of the extension uses (connection-
          // status.ts's PAIRING_DEEP_LINK/GET_APP_URL, options.ts's
          // `openDeepLink`): `browser.tabs.create` in a click handler, wrapped
          // in try/catch. A raw custom-scheme anchor is unverified cross-
          // browser and can silently no-op.
          const link = button('btn btn--quiet', 'Generate in the app');
          link.addEventListener('click', () => void openGenerateLink());
          host.append(link);
        }
      }
      return;
    }

    const current = selected();
    if (!current) return;

    if (candidates.length > 1) {
      const sourceLabel = el('label', 'field-label', 'Source');
      const sourceSelect = document.createElement('select');
      candidates.forEach((c, i) => {
        const opt = document.createElement('option');
        opt.value = String(i);
        opt.textContent = c.label;
        sourceSelect.append(opt);
      });
      sourceSelect.value = String(selectedIndex);
      sourceSelect.addEventListener('change', () => {
        selectedIndex = Number(sourceSelect.value) || 0;
        if (kind === 'cover-letter' && !selected()?.hasCoverLetter) kind = 'resume';
        render();
      });
      sourceLabel.append(sourceSelect);
      host.append(sourceLabel);
    }

    const kindRow = el('div', 'kind-row');
    const resumeBtn = button('btn btn--small', 'Résumé');
    resumeBtn.setAttribute('aria-pressed', String(kind === 'resume'));
    if (kind === 'resume') resumeBtn.classList.add('btn--primary');
    resumeBtn.addEventListener('click', () => {
      kind = 'resume';
      render();
    });
    kindRow.append(resumeBtn);

    const letterBtn = button('btn btn--small', 'Cover letter');
    letterBtn.disabled = !current.hasCoverLetter;
    letterBtn.title = current.hasCoverLetter
      ? ''
      : 'This source has no cover letter — generate one in the app first.';
    letterBtn.setAttribute('aria-pressed', String(kind === 'cover-letter'));
    if (kind === 'cover-letter') letterBtn.classList.add('btn--primary');
    letterBtn.addEventListener('click', () => {
      kind = 'cover-letter';
      render();
    });
    kindRow.append(letterBtn);
    host.append(kindRow);

    const templateLabel = el('label', 'field-label', 'Template');
    const templateSelect = document.createElement('select');
    for (const [id, label] of Object.entries(TEMPLATE_LABELS)) {
      const opt = document.createElement('option');
      opt.value = id;
      opt.textContent = label;
      templateSelect.append(opt);
    }
    templateSelect.value = templateId;
    templateSelect.addEventListener('change', () => {
      templateId = templateSelect.value;
    });
    templateLabel.append(templateSelect);
    host.append(templateLabel);

    if (kind === 'cover-letter') {
      const layoutLabel = el('label', 'field-label', 'Letter layout');
      const layoutSelect = document.createElement('select');
      for (const [id, label] of Object.entries(LETTER_LAYOUT_LABELS)) {
        const opt = document.createElement('option');
        opt.value = id;
        opt.textContent = label;
        layoutSelect.append(opt);
      }
      layoutSelect.value = letterLayoutId;
      layoutSelect.addEventListener('change', () => {
        letterLayoutId = layoutSelect.value;
      });
      layoutLabel.append(layoutSelect);
      host.append(layoutLabel);

      const actions = el('div', 'action-row');
      const pasteBtn = button('btn btn--primary', 'Paste cover letter…');
      pasteBtn.disabled = busy;
      pasteBtn.addEventListener('click', () => void doOpenPastePicker());
      actions.append(pasteBtn);
      const copyBtn = button('btn btn--quiet', 'Copy cover letter');
      copyBtn.disabled = busy;
      copyBtn.addEventListener('click', () => void doCopy());
      actions.append(copyBtn);
      host.append(actions);

      if (pastePickerOpen) host.append(renderPastePicker());
    } else {
      const formatLabel = el('label', 'field-label', 'Format');
      const formatSelect = document.createElement('select');
      for (const [value, label] of [
        ['pdf', 'PDF'],
        ['docx', 'DOCX'],
      ] as const) {
        const opt = document.createElement('option');
        opt.value = value;
        opt.textContent = label;
        formatSelect.append(opt);
      }
      formatSelect.value = format;
      formatSelect.addEventListener('change', () => {
        format = formatSelect.value === 'docx' ? 'docx' : 'pdf';
      });
      formatLabel.append(formatSelect);
      host.append(formatLabel);

      const attachBtn = button('btn btn--primary', 'Attach résumé to this page');
      attachBtn.disabled = busy;
      attachBtn.addEventListener('click', () => void doAttach());
      host.append(attachBtn);
    }

    if (statusText) {
      host.append(el('p', `msg msg--${statusTone === 'muted' ? 'muted' : statusTone}`, statusText));
    }
  }

  function renderPastePicker(): HTMLElement {
    const wrap = el('div', 'picker');
    wrap.append(el('p', 'field-label', 'Paste into…'));
    const rows = (state?.rows ?? []).filter((r) => r.field !== null && !state?.pageChanged);
    if (rows.length === 0) {
      wrap.append(el('p', 'msg msg--muted', 'No form fields found on this page to paste into.'));
      return wrap;
    }
    for (const row of rows) {
      const rowBtn = button('btn btn--small btn--quiet picker__row', row.question);
      rowBtn.disabled = busy;
      rowBtn.addEventListener('click', () => void doPasteInto(row.id));
      wrap.append(rowBtn);
    }
    const cancel = button('btn btn--small btn--quiet', 'Cancel');
    cancel.addEventListener('click', () => {
      pastePickerOpen = false;
      render();
    });
    wrap.append(cancel);
    return wrap;
  }

  async function doOpenPastePicker(): Promise<void> {
    pastePickerOpen = true;
    setStatus('', 'muted');
    render();
  }

  async function fetchCoverLetterText(): Promise<{ text: string } | { error: string }> {
    const current = selected();
    if (!current) return { error: 'Nothing selected.' };
    const req: PopupRequest = {
      kind: 'documentExportText',
      source: current.source,
      templateId,
      ...(letterLayoutId !== 'classic' ? { letterLayoutId } : {}),
    };
    const res = await deps.send(req);
    if (!res.ok) return { error: res.error };
    if (res.kind !== 'documentExportText') return { error: 'Unexpected response — please retry.' };
    return { text: res.text };
  }

  async function doCopy(): Promise<void> {
    busy = true;
    setStatus('Fetching…', 'muted');
    render();
    try {
      const out = await fetchCoverLetterText();
      if ('error' in out) {
        setStatus(out.error, 'err');
        return;
      }
      const ok = await deps.copy(out.text);
      setStatus(ok ? 'Copied.' : 'Could not copy — try again.', ok ? 'ok' : 'err');
    } catch (err) {
      setStatus(err instanceof Error ? err.message : String(err), 'err');
    } finally {
      busy = false;
      render();
    }
  }

  async function doPasteInto(rowId: string): Promise<void> {
    const row = state?.rows.find((r) => r.id === rowId);
    const field = row?.field;
    if (!row || !field) return;
    // Bind this paste to the followed tab captured BEFORE the (possibly
    // slow) cover-letter export below — `follow()` can re-target the panel
    // at a different tab during that wait, and `deps.send` has no tabId of
    // its own to bind to (PR review round 2).
    const capturedGeneration = deps.getFollowGeneration();
    const capturedTabId = state?.tabId ?? null;
    busy = true;
    pastePickerOpen = false;
    setStatus('Fetching…', 'muted');
    render();
    try {
      const out = await fetchCoverLetterText();
      if ('error' in out) {
        setStatus(out.error, 'err');
        return;
      }
      if (
        deps.getFollowGeneration() !== capturedGeneration ||
        (state?.tabId ?? null) !== capturedTabId
      ) {
        setStatus('The followed tab changed — please retry.', 'err');
        return;
      }
      const res = await deps.send(
        field.kind === 'filled'
          ? {
              kind: 'answerReplace',
              question: row.question,
              index: field.index,
              count: field.count,
              text: out.text,
              expectedValue: field.currentText,
            }
          : {
              kind: 'answerFill',
              question: row.question,
              index: field.index,
              count: field.count,
              answer: out.text,
            }
      );
      if (!res.ok) {
        setStatus(res.error, 'err');
        return;
      }
      let result: { filled: boolean; error?: string } | null = null;
      if (res.kind === 'answerReplace' || res.kind === 'answerFill') result = res.result;
      setStatus(
        result?.filled
          ? 'Pasted into the field.'
          : (result?.error ?? 'Could not paste into that field.'),
        result?.filled ? 'ok' : 'err'
      );
    } catch (err) {
      setStatus(err instanceof Error ? err.message : String(err), 'err');
    } finally {
      busy = false;
      render();
    }
  }

  async function doAttach(): Promise<void> {
    const current = selected();
    if (!current) return;
    const proceed = await deps.confirmAttach(deps.currentHost());
    if (!proceed) return;
    busy = true;
    setStatus('Attaching…', 'muted');
    render();
    try {
      const res = await deps.send({
        kind: 'documentAttach',
        source: current.source,
        templateId,
        format,
      });
      if (!res.ok) {
        setStatus(res.error, 'err');
        return;
      }
      if (res.kind !== 'documentAttach') {
        setStatus('Unexpected response — please retry.', 'err');
        return;
      }
      setStatus(
        res.result.attached
          ? `Attached ${res.result.filename ?? 'the résumé'} — review it on the page.`
          : (res.result.reason ?? 'Could not attach the résumé.'),
        res.result.attached ? 'ok' : 'err'
      );
    } catch (err) {
      setStatus(err instanceof Error ? err.message : String(err), 'err');
    } finally {
      busy = false;
      render();
    }
  }

  async function refresh(): Promise<void> {
    generation += 1;
    const myGeneration = generation;
    loading = true;
    setStatus('', 'muted');
    render();
    try {
      const res = await deps.send({ kind: 'documentsList' });
      if (myGeneration !== generation) return;
      loading = false;
      if (!res.ok) {
        candidates = [];
        setStatus(res.error, 'err');
        render();
        return;
      }
      if (res.kind !== 'documentsList') {
        // A kind mismatch is a terminal outcome too — clear `loading` so the
        // empty state can never sit on a phantom "Loading…" (#1225).
        candidates = [];
        setStatus('Unexpected response — please retry.', 'err');
        render();
        return;
      }
      lastUrl = res.url;
      if (lastUrl) deps.onUrlResolved?.(lastUrl);
      if (!res.result.ok) {
        candidates = [];
        setStatus(res.result.error, 'err');
        render();
        return;
      }
      const data = parseDocumentsResourceData(res.result.data);
      candidates = buildCandidates(data, res.url);
      selectedIndex = 0;
      if (candidates.length === 0) {
        setStatus('No documents yet for this job.', 'muted');
      } else {
        setStatus('', 'muted');
        if (kind === 'cover-letter' && !selected()?.hasCoverLetter) kind = 'resume';
      }
      render();
    } catch (err) {
      if (myGeneration !== generation) return;
      loading = false;
      candidates = [];
      setStatus(err instanceof Error ? err.message : String(err), 'err');
      render();
    }
  }

  function reset(reason = ''): void {
    generation += 1;
    loading = false;
    candidates = [];
    pastePickerOpen = false;
    busy = false;
    lastUrl = '';
    setStatus(reason, 'muted');
    render();
  }

  render();

  return {
    render: (next) => {
      state = next;
      // A field-set change invalidates any open picker built from the
      // PREVIOUS state — never leave a stale row list clickable.
      if (pastePickerOpen) render();
    },
    refresh: () => void refresh(),
    reset,
  };
}
