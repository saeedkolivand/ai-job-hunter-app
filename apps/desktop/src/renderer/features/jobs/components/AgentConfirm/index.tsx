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
 *
 * Every gated Write tool in the app carries exactly ONE content property (the
 * job it belongs to is fixed by the run — see `save_resume_schema`/
 * `save_cover_letter_schema`), so a tool is described by the NAME of that
 * property rather than by a bespoke pair of extract/rebuild functions per tool.
 * `save_resume` is the improve-resume flow's only write, and it is reached from
 * the prep flow too.
 */
const KNOWN_TOOL_KEYS: Record<
  string,
  { summaryKey: string; contentLabelKey: string; contentKey: string }
> = {
  save_cover_letter: {
    summaryKey: 'jobs.prep.confirm.tools.saveCoverLetter.summary',
    contentLabelKey: 'jobs.prep.confirm.tools.saveCoverLetter.contentLabel',
    contentKey: 'coverLetterText',
  },
  save_resume: {
    summaryKey: 'jobs.prep.confirm.tools.saveResume.summary',
    contentLabelKey: 'jobs.prep.confirm.tools.saveResume.contentLabel',
    contentKey: 'resumeText',
  },
};

/**
 * Extract a known tool's editable text from its args, or `null` when the shape
 * isn't what's expected — which falls the card back to read-only JSON. That
 * fallback matters more than it looks: an editable field prefilled with `''`
 * because the property was missing invites the user to approve an EMPTY
 * document over the one they have.
 *
 * The text arrives already clamped for display by the gate
 * (`display_cap_for`: 40 000 chars for `save_resume`, 20 000 for the letter) —
 * to each tool's OWN content cap, so what is shown and edited here is exactly
 * what the tool would persist. Nothing is re-clamped renderer-side.
 */
function knownTextOf(known: { contentKey: string } | undefined, args: unknown): string | null {
  if (!known || !args || typeof args !== 'object') return null;
  const value = (args as Record<string, unknown>)[known.contentKey];
  return typeof value === 'string' ? value : null;
}

/** Rebuild `args` with the user's edited text — content only, every other field
 *  passes through untouched (the shell re-validates against the tool's schema
 *  and refuses any undeclared key). */
function withEditedText(known: { contentKey: string }, args: unknown, text: string): unknown {
  const base = args && typeof args === 'object' ? (args as Record<string, unknown>) : {};
  return { ...base, [known.contentKey]: text };
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
 * content before it executes. Rendered inline by whichever panel is hosting the
 * run while a confirm is pending; the run is genuinely blocked until this
 * resolves.
 */
export function AgentConfirm({ jobId, confirm, onResolved }: AgentConfirmProps) {
  const { t } = useTranslation();
  const confirmMutation = useAgentConfirm();
  const headingRef = useRef<HTMLHeadingElement>(null);
  const textAreaRef = useRef<HTMLTextAreaElement>(null);
  const originalTextRef = useRef('');

  const known = KNOWN_TOOL_KEYS[confirm.tool];
  /** A known tool whose args really carried its content — the only case that
   *  earns the editable field (see {@link knownTextOf}). */
  const editable = knownTextOf(known, confirm.args) !== null;
  const [editing, setEditing] = useState(false);
  const [text, setText] = useState(knownTextOf(known, confirm.args) ?? '');
  const [unavailable, setUnavailable] = useState(false);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);

  // A fresh confirm request (new callId) is a new decision point: reset any
  // stale edit/unavailable/error state from a previous call and move focus here.
  useEffect(() => {
    const original = knownTextOf(KNOWN_TOOL_KEYS[confirm.tool], confirm.args) ?? '';
    originalTextRef.current = original;
    setEditing(false);
    setText(original);
    setUnavailable(false);
    setErrorMessage(null);
    headingRef.current?.focus();
  }, [confirm.callId, confirm.args, confirm.tool]);

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

          {known && editable ? (
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
                  editing && known ? 'approveEdited' : 'approve',
                  editing && known ? withEditedText(known, confirm.args, text) : undefined
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
            {editable && !editing && (
              <Button
                variant="ghost"
                disabled={busy}
                onClick={handleEdit}
                className="ml-auto gap-1.5"
              >
                {t('jobs.prep.confirm.edit')}
              </Button>
            )}
            {editable && editing && (
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
