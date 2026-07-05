import { useEffect, useRef, useState } from 'react';

import type { AgentConfirmPayload } from '@ajh/shared';
import { useTranslation } from '@ajh/translations';
import { Button, GlassCard, TextArea } from '@ajh/ui';

import { useAgentConfirm } from '@/services';

/**
 * Tools with a known content shape get a friendly summary + an editable field.
 * Anything else falls back to a generic summary and read-only JSON — the
 * confirm UI stays generic over `{ tool, args }` (new gated Write tools need
 * no renderer change to at least render safely).
 */
const KNOWN_TOOL_KEYS: Record<string, { summaryKey: string; contentLabelKey: string }> = {
  save_cover_letter: {
    summaryKey: 'jobs.prep.confirm.tools.saveCoverLetter.summary',
    contentLabelKey: 'jobs.prep.confirm.tools.saveCoverLetter.contentLabel',
  },
};

/** Extract the editable cover-letter text from `save_cover_letter`'s args, or
 *  `null` when the shape isn't what's expected (falls back to read-only JSON). */
function coverLetterTextOf(args: unknown): string | null {
  if (
    args &&
    typeof args === 'object' &&
    'coverLetterText' in args &&
    typeof args.coverLetterText === 'string'
  ) {
    return (args as { coverLetterText: string }).coverLetterText;
  }
  return null;
}

/** Rebuild `args` with the user's edited cover-letter text — content only,
 *  every other field passes through untouched (the shell re-validates). */
function withEditedCoverLetterText(args: unknown, text: string): unknown {
  const base = args && typeof args === 'object' ? (args as Record<string, unknown>) : {};
  return { ...base, coverLetterText: text };
}

export interface AgentConfirmProps {
  jobId: string;
  confirm: AgentConfirmPayload;
  /** Called once the pending call is actually resolved (`{ ok: true }`) —
   *  `'APPROVE'` for approve/approveEdited, `'DENY'` for deny. NOT called on
   *  `{ ok: false }` (the caller shows the "no longer available" message
   *  in place instead — see `unavailable` below). Move focus to a stable
   *  in-modal node here — this component (and its heading) unmounts right
   *  after, since the caller clears the pending confirm. */
  onResolved: (event: 'APPROVE' | 'DENY') => void;
}

/**
 * The Phase-3 human-in-the-loop confirm prompt: renders a pending
 * `confirm_request`'s tool + args as DATA (never markup — `args` is untrusted
 * model output) and lets the user Approve, Deny, or Edit-then-approve the
 * content before it executes. Rendered inline by `PrepApplicationPanel` while
 * a confirm is pending; the run is genuinely blocked until this resolves.
 */
export function AgentConfirm({ jobId, confirm, onResolved }: AgentConfirmProps) {
  const { t } = useTranslation();
  const confirmMutation = useAgentConfirm();
  const headingRef = useRef<HTMLHeadingElement>(null);
  const textAreaRef = useRef<HTMLTextAreaElement>(null);
  const originalTextRef = useRef('');

  const known = KNOWN_TOOL_KEYS[confirm.tool];
  const [editing, setEditing] = useState(false);
  const [text, setText] = useState(coverLetterTextOf(confirm.args) ?? '');
  const [unavailable, setUnavailable] = useState(false);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);

  // A fresh confirm request (new callId) is a new decision point: reset any
  // stale edit/unavailable/error state from a previous call and move focus here.
  useEffect(() => {
    const original = coverLetterTextOf(confirm.args) ?? '';
    originalTextRef.current = original;
    setEditing(false);
    setText(original);
    setUnavailable(false);
    setErrorMessage(null);
    headingRef.current?.focus();
  }, [confirm.callId, confirm.args]);

  // Edit is a real decision point too — move focus into the now-editable field
  // rather than leaving it on the (now relabeled) Edit-adjacent button.
  useEffect(() => {
    if (editing) textAreaRef.current?.focus();
  }, [editing]);

  // `{ ok: false }` keeps this component mounted (only the actions disappear)
  // — reclaim focus onto the heading so it doesn't fall through to <body>.
  useEffect(() => {
    if (unavailable) headingRef.current?.focus();
  }, [unavailable]);

  const busy = confirmMutation.isPending;

  const resolve = async (decision: 'approve' | 'approveEdited' | 'deny', editedArgs?: unknown) => {
    setErrorMessage(null);
    try {
      const { ok } = await confirmMutation.mutateAsync({
        jobId,
        callId: confirm.callId,
        decision,
        editedArgs,
      });
      if (!ok) {
        setUnavailable(true);
        return;
      }
      // Move focus to a stable node the parent owns BEFORE it unmounts us
      // (the state update that removes this component hasn't committed yet,
      // so `headingRef` — the best in-modal target we control — is still
      // live here; the caller then re-parks focus on ITS stable node once we're gone).
      headingRef.current?.focus();
      onResolved(decision === 'deny' ? 'DENY' : 'APPROVE');
    } catch {
      // A transport-level failure (distinct from the modeled `{ ok: false }`)
      // — surface it and leave the actions enabled so the user can retry,
      // rather than silently re-enabling with no feedback.
      setErrorMessage(t('jobs.prep.confirm.error'));
    }
  };

  const handleEdit = () => setEditing(true);
  const handleRevert = () => {
    setText(originalTextRef.current);
    setEditing(false);
  };

  const summary = known
    ? t(known.summaryKey)
    : t('jobs.prep.confirm.genericSummary', { tool: confirm.tool });

  return (
    <GlassCard
      tone="surface"
      role="region"
      aria-labelledby="agent-confirm-heading"
      className="space-y-3 !p-4 border-l-2 border-[var(--color-brand)]"
    >
      <h3
        ref={headingRef}
        id="agent-confirm-heading"
        tabIndex={-1}
        className="text-xs font-semibold text-foreground/85 outline-none"
      >
        {t('jobs.prep.confirm.heading')}
      </h3>
      <p className="text-caption text-foreground/70">{summary}</p>

      {unavailable ? (
        <p role="status" className="text-caption text-foreground/70">
          {t('jobs.prep.confirm.unavailable')}
        </p>
      ) : (
        <>
          <p className="text-caption text-foreground/70">{t('jobs.prep.confirm.hint')}</p>

          {known ? (
            <TextArea
              ref={textAreaRef}
              value={text}
              onChange={(e) => setText(e.target.value)}
              readOnly={!editing}
              rows={10}
              aria-label={t(known.contentLabelKey)}
              className={!editing ? 'opacity-70' : undefined}
            />
          ) : (
            <TextArea
              value={JSON.stringify(confirm.args, null, 2)}
              readOnly
              rows={10}
              aria-label={t('jobs.prep.confirm.rawArgsLabel')}
              className="font-mono text-[10px] opacity-70"
            />
          )}

          {errorMessage && (
            <p role="alert" className="text-caption text-red-300">
              {errorMessage}
            </p>
          )}
          {busy && (
            <p role="status" className="text-caption text-foreground/70">
              {t('jobs.prep.confirm.submitting')}
            </p>
          )}

          <div className="flex flex-wrap items-center gap-2">
            <Button
              variant="primary"
              loading={busy}
              title={t('jobs.prep.confirm.approveHint')}
              onClick={() =>
                void resolve(
                  editing ? 'approveEdited' : 'approve',
                  editing ? withEditedCoverLetterText(confirm.args, text) : undefined
                )
              }
              className="gap-1.5"
            >
              {editing ? t('jobs.prep.confirm.approveEdited') : t('jobs.prep.confirm.approve')}
            </Button>
            <Button
              variant="glass"
              loading={busy}
              title={t('jobs.prep.confirm.denyHint')}
              onClick={() => void resolve('deny')}
              className="gap-1.5"
            >
              {t('jobs.prep.confirm.deny')}
            </Button>
            {known && !editing && (
              <Button
                variant="ghost"
                disabled={busy}
                onClick={handleEdit}
                className="ml-auto gap-1.5"
              >
                {t('jobs.prep.confirm.edit')}
              </Button>
            )}
            {known && editing && (
              <Button
                variant="ghost"
                disabled={busy}
                onClick={handleRevert}
                className="ml-auto gap-1.5"
              >
                {t('jobs.prep.confirm.revert')}
              </Button>
            )}
          </div>
        </>
      )}
    </GlassCard>
  );
}
