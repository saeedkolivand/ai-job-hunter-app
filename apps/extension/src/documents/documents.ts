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

import { LETTER_LAYOUT_LABELS, TEMPLATE_LABELS } from '@ajh/shared/ipc';

import type { AnswerState } from '../lib/answer-state';
import { appendEmptyState, button, el, openDeepLink, selectField } from '../lib/dom';
import type { PopupRequest, PopupResponse } from '../lib/messages';
import { buildKindRow, buildPastePicker, type DocumentKind } from './controls';
import { buildCandidates, type DocumentCandidate, parseDocumentsResourceData } from './resource';

export * from './resource';

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

export function mountDocuments(host: HTMLElement, deps: DocumentsDeps): DocumentsView {
  let candidates: DocumentCandidate[] = [];
  let selectedIndex = 0;
  let kind: DocumentKind = 'resume';
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
  /** Terminal failure of a refresh: no candidates, the error on the status line. */
  const fail = (message: string): void => {
    candidates = [];
    setStatus(message, 'err');
    render();
  };
  /** Mark the tab busy around `task`; a throw lands on the status line. */
  async function runBusy(pending: string, task: () => Promise<void>): Promise<void> {
    busy = true;
    setStatus(pending, 'muted');
    render();
    try {
      await task();
    } catch (err) {
      setStatus(err instanceof Error ? err.message : String(err), 'err');
    } finally {
      busy = false;
      render();
    }
  }

  function selected(): DocumentCandidate | null {
    return candidates[selectedIndex] ?? null;
  }

  function render(): void {
    host.replaceChildren();

    if (candidates.length === 0) {
      appendEmptyState(
        host,
        loading,
        statusText,
        statusTone === 'muted' && lastUrl
          ? {
              label: 'Generate in the app',
              onClick: () => void openDeepLink(`ajh://generate?url=${encodeURIComponent(lastUrl)}`),
            }
          : null
      );
      return;
    }

    const current = selected();
    if (!current) return;

    if (candidates.length > 1) {
      const options = candidates.map((c, i) => [String(i), c.label] as const);
      host.append(
        selectField('Source', options, String(selectedIndex), (value) => {
          selectedIndex = Number(value) || 0;
          if (kind === 'cover-letter' && !selected()?.hasCoverLetter) kind = 'resume';
          render();
        })
      );
    }

    host.append(
      buildKindRow(kind, current.hasCoverLetter, (next) => {
        kind = next;
        render();
      })
    );
    host.append(
      selectField('Template', Object.entries(TEMPLATE_LABELS), templateId, (value) => {
        templateId = value;
      })
    );

    if (kind === 'cover-letter') {
      host.append(
        selectField(
          'Letter layout',
          Object.entries(LETTER_LAYOUT_LABELS),
          letterLayoutId,
          (value) => {
            letterLayoutId = value;
          }
        )
      );
      const actions = el('div', 'action-row');
      const pasteBtn = button('btn btn--primary', 'Paste cover letter…');
      pasteBtn.disabled = busy;
      pasteBtn.addEventListener('click', openPastePicker);
      const copyBtn = button('btn btn--quiet', 'Copy cover letter');
      copyBtn.disabled = busy;
      copyBtn.addEventListener('click', () => void doCopy());
      actions.append(pasteBtn, copyBtn);
      host.append(actions);

      if (pastePickerOpen) {
        host.append(
          buildPastePicker(
            state,
            busy,
            (rowId) => void doPasteInto(rowId),
            () => {
              pastePickerOpen = false;
              render();
            }
          )
        );
      }
    } else {
      const formats = [
        ['pdf', 'PDF'],
        ['docx', 'DOCX'],
      ] as const;
      host.append(
        selectField('Format', formats, format, (value) => {
          format = value === 'docx' ? 'docx' : 'pdf';
        })
      );
      const attachBtn = button('btn btn--primary', 'Attach résumé to this page');
      attachBtn.disabled = busy;
      attachBtn.addEventListener('click', () => void doAttach());
      host.append(attachBtn);
    }

    if (statusText) {
      host.append(el('p', `msg msg--${statusTone}`, statusText));
    }
  }

  function openPastePicker(): void {
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

  const doCopy = (): Promise<void> =>
    runBusy('Fetching…', async () => {
      const out = await fetchCoverLetterText();
      if ('error' in out) return setStatus(out.error, 'err');
      const ok = await deps.copy(out.text);
      setStatus(ok ? 'Copied.' : 'Could not copy — try again.', ok ? 'ok' : 'err');
    });

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
    pastePickerOpen = false;
    await runBusy('Fetching…', async () => {
      const out = await fetchCoverLetterText();
      if ('error' in out) return setStatus(out.error, 'err');
      if (
        deps.getFollowGeneration() !== capturedGeneration ||
        (state?.tabId ?? null) !== capturedTabId
      ) {
        return setStatus('The followed tab changed — please retry.', 'err');
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
      if (!res.ok) return setStatus(res.error, 'err');
      let result: { filled: boolean; error?: string } | null = null;
      if (res.kind === 'answerReplace' || res.kind === 'answerFill') result = res.result;
      setStatus(
        result?.filled
          ? 'Pasted into the field.'
          : (result?.error ?? 'Could not paste into that field.'),
        result?.filled ? 'ok' : 'err'
      );
    });
  }

  async function doAttach(): Promise<void> {
    const current = selected();
    if (!current) return;
    const proceed = await deps.confirmAttach(deps.currentHost());
    if (!proceed) return;
    await runBusy('Attaching…', async () => {
      const res = await deps.send({
        kind: 'documentAttach',
        source: current.source,
        templateId,
        format,
      });
      if (!res.ok) return setStatus(res.error, 'err');
      if (res.kind !== 'documentAttach') {
        return setStatus('Unexpected response — please retry.', 'err');
      }
      setStatus(
        res.result.attached
          ? `Attached ${res.result.filename ?? 'the résumé'} — review it on the page.`
          : (res.result.reason ?? 'Could not attach the résumé.'),
        res.result.attached ? 'ok' : 'err'
      );
    });
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
      if (!res.ok) return fail(res.error);
      if (res.kind !== 'documentsList') {
        // A kind mismatch is a terminal outcome too — clear `loading` so the
        // empty state can never sit on a phantom "Loading…" (#1225).
        return fail('Unexpected response — please retry.');
      }
      lastUrl = res.url;
      if (lastUrl) deps.onUrlResolved?.(lastUrl);
      if (!res.result.ok) return fail(res.result.error);
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
      fail(err instanceof Error ? err.message : String(err));
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
