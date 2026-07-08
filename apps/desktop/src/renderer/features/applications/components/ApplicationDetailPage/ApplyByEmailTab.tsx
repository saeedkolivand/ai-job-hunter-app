import { Briefcase, Check, ClipboardCopy, FileText, Mail, Sparkles } from 'lucide-react';
import { AnimatePresence } from 'motion/react';
import { useEffect, useRef, useState } from 'react';

import type { AiGenerationRecord, Application } from '@ajh/shared';
import { useTranslation } from '@ajh/translations';
import { Button, CardSkeleton, EmptyState, Input, RowSkeleton, StreamingText } from '@ajh/ui';

import {
  RewritePopover,
  type RewriteTarget,
} from '@/components/generation/EditableOutput/RewritePopover';
import { useCanUseAI, useSelectedModel } from '@/components/ui/ModelSelector';
import { useDefaultResumeId } from '@/hooks/useDefaultResumeId';
import { generateApplicationEmail, type GenerationMeta } from '@/lib/generate';
import { getSelectionOffsets } from '@/lib/selection-offsets';
import { COPY_FEEDBACK_MS } from '@/lib/timings';
import { useDocuments, useDocumentText, useUpdateApplication } from '@/services';

import { extractRecipient } from '../../lib/extract-recipient';

interface Props {
  application: Application;
  matchingGenerations: AiGenerationRecord[];
}

/** The two independently-rewritable fields of the draft. */
type EmailField = 'subject' | 'body';

/** A rewrite frozen at trigger time — which field, the splice range, the snapshot
 *  it splices back into on Accept, the rewrite target, and the anchor button. */
interface FrozenRewrite {
  field: EmailField;
  start: number;
  end: number;
  snapshot: string;
  target: RewriteTarget;
  anchorEl: HTMLElement;
}

/** Split raw model output per the OUTPUT CONTRACT: line 1 is "Subject: …". */
function splitEmail(raw: string): { subject: string; body: string } {
  const firstLine = raw.split('\n')[0] ?? '';
  const m = /^Subject:\s*(.*)$/i.exec(firstLine);
  if (!m) return { subject: '', body: raw.trim() };
  return {
    subject: m[1]?.trim() ?? '',
    body: raw.slice(firstLine.length).replace(/^\n/, '').trim(),
  };
}

/**
 * "Apply by email" tab — generates a short application email to send directly
 * to an employer contact. Recipient fields are prefilled from the job description
 * (heuristic extractor, user-editable) and persisted to the Application.
 * Generation streams through the shared AI pipeline; the user sends from their
 * own mail client via the mailto button.
 */
export function ApplyByEmailTab({ application, matchingGenerations }: Props) {
  const { t } = useTranslation();
  const model = useSelectedModel();
  const { canUse } = useCanUseAI();

  const { isLoading: docsLoading } = useDocuments();
  const defaultResumeId = useDefaultResumeId();
  const resumeQuery = useDocumentText(defaultResumeId);
  const updateApplication = useUpdateApplication();

  const saved = matchingGenerations[0];
  const jobDesc = (application.jobDescription ?? '').trim() || (saved?.jobAd ?? '').trim();
  const resume = (resumeQuery.data ?? '').trim() || (saved?.resumeText ?? '').trim();

  const meta: GenerationMeta = saved
    ? {
        candidateName: saved.candidateName,
        jobTitle: saved.jobTitle,
        companyName: saved.companyName,
        targetLanguage: saved.targetLanguage,
        resumeLanguage: saved.resumeLanguage,
        jobAdLanguage: saved.jobAdLanguage,
        mismatch: saved.mismatch,
        topRequirements: saved.topRequirements,
      }
    : {
        candidateName: '',
        jobTitle: application.title,
        companyName: application.company,
        targetLanguage: 'en',
        resumeLanguage: 'en',
        jobAdLanguage: 'en',
        mismatch: false,
        topRequirements: [],
      };

  // Recipient fields seeded from Application, then prefilled once from extractor
  const [recipientName, setRecipientName] = useState(application.recipientName ?? '');
  const [recipientEmail, setRecipientEmail] = useState(application.recipientEmail ?? '');
  const [emailError, setEmailError] = useState<string | null>(null);

  // Prefill from job description when both fields are empty. The guard
  // (`recipientName || recipientEmail`) prevents re-applying once filled.
  // Including jobDesc in deps also handles the case where it loads after mount.
  useEffect(() => {
    if (recipientName || recipientEmail) return;
    if (!jobDesc) return;
    const extracted = extractRecipient(jobDesc);
    if (extracted.name) setRecipientName(extracted.name);
    if (extracted.email) setRecipientEmail(extracted.email);
  }, [jobDesc, recipientEmail, recipientName]);

  // Generation state
  const [streamText, setStreamText] = useState('');
  const [isGenerating, setIsGenerating] = useState(false);
  const [genError, setGenError] = useState<string | null>(null);
  const abortRef = useRef<AbortController | null>(null);
  // Mutable draft populated once generation completes, so a select-to-rewrite
  // result can be spliced back in. `null` while streaming / before the first
  // generation — the live split from `streamText` is used then.
  const [email, setEmail] = useState<{ subject: string; body: string } | null>(null);

  // Abort any in-flight stream on unmount to prevent quota burn on tab change.
  useEffect(
    () => () => {
      abortRef.current?.abort();
    },
    []
  );

  // While streaming (or before the first generation) split the live stream; once
  // generation settles, the editable `email` draft wins so rewrites persist.
  const live = splitEmail(streamText);
  const subject = email?.subject ?? live.subject;
  const body = email?.body ?? live.body;
  const hasDraft = streamText.trim().length > 0;
  const canGenerate = canUse && !!model && !!resume && !!jobDesc && !isGenerating;
  const canRewrite = canUse && !!model;

  const handleGenerate = async () => {
    if (!canGenerate) return;
    abortRef.current?.abort();
    abortRef.current = new AbortController();
    setIsGenerating(true);
    setStreamText('');
    setEmail(null);
    setGenError(null);
    try {
      const full = await generateApplicationEmail({
        resume,
        jobAd: jobDesc,
        meta,
        model,
        recipientName: recipientName.trim() || undefined,
        recipientEmail: recipientEmail.trim() || undefined,
        companyBrief: saved?.companyBrief ?? '',
        signal: abortRef.current.signal,
        onToken: (tok) => setStreamText((prev) => prev + tok),
      });
      // Freeze the final draft into editable state so rewrites can splice into it.
      setEmail(splitEmail(full));
    } catch (err) {
      if (err instanceof Error && err.name === 'AbortError') return;
      setGenError(t('applications.detail.email.genError'));
    } finally {
      setIsGenerating(false);
    }
  };

  // Copy feedback: show "Copied!" for 2 s then reset.
  const [copied, setCopied] = useState(false);
  const copyTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  const handleCopy = () => {
    const full = subject ? `Subject: ${subject}\n\n${body}` : body;
    void navigator.clipboard
      .writeText(full)
      .then(() => {
        setCopied(true);
        if (copyTimerRef.current) clearTimeout(copyTimerRef.current);
        copyTimerRef.current = setTimeout(() => setCopied(false), 2000);
      })
      .catch(() => {});
  };

  // Subject-only copy — writes JUST the subject (no "Subject:" prefix, no body)
  // so it drops straight into a mail client's Subject field. Its own flag so it
  // never collides with the whole-email Copy button above.
  const [subjectCopied, setSubjectCopied] = useState(false);
  const subjectCopyTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  const handleCopySubject = () => {
    void navigator.clipboard
      .writeText(subject)
      .then(() => {
        setSubjectCopied(true);
        if (subjectCopyTimerRef.current) clearTimeout(subjectCopyTimerRef.current);
        subjectCopyTimerRef.current = setTimeout(() => setSubjectCopied(false), COPY_FEEDBACK_MS);
      })
      .catch(() => {});
  };

  // Select-to-rewrite (mirrors ApplicationQuestionsModal): one frozen rewrite at
  // a time. The selected span inside the field's <p> — or the whole field when
  // nothing is selected — is captured at trigger time and spliced back on Accept.
  const subjectRef = useRef<HTMLParagraphElement | null>(null);
  const bodyRef = useRef<HTMLParagraphElement | null>(null);
  const [frozen, setFrozen] = useState<FrozenRewrite | null>(null);

  const openRewrite = (field: EmailField, trigger: HTMLElement) => {
    const text = field === 'subject' ? subject : body;
    const container = field === 'subject' ? subjectRef.current : bodyRef.current;
    const offsets = container ? getSelectionOffsets(container) : null;
    const start = offsets?.start ?? 0;
    const end = offsets?.end ?? text.length;
    setFrozen({
      field,
      start,
      end,
      snapshot: text,
      anchorEl: trigger,
      target: {
        selection: text.slice(start, end),
        before: text.slice(0, start),
        after: text.slice(end),
      },
    });
  };

  const closeRewrite = () => {
    const trigger = frozen?.anchorEl;
    setFrozen(null);
    trigger?.focus();
  };

  // Splice the accepted replacement back into the frozen snapshot and commit it to
  // the editable draft. Local-only (the draft isn't persisted to the Application),
  // so there's no save/rollback to coordinate — unlike the answers surface.
  const acceptRewrite = (replacement: string) => {
    if (!frozen) return;
    const { field, start, end, snapshot } = frozen;
    const next = snapshot.slice(0, start) + replacement + snapshot.slice(end);
    setFrozen(null);
    setEmail((prev) => ({ ...(prev ?? live), [field]: next }));
  };

  const mailtoHref =
    recipientEmail.trim() && subject && body
      ? `mailto:${encodeURIComponent(recipientEmail.trim())}?subject=${encodeURIComponent(subject)}&body=${encodeURIComponent(body)}`
      : undefined;

  const persistName = () => {
    const val = recipientName.trim();
    if (val !== (application.recipientName ?? '')) {
      updateApplication.mutate({ id: application.id, recipientName: val });
    }
  };

  const persistEmail = (value: string) => {
    const val = value.trim();
    if (val === (application.recipientEmail ?? '')) return;
    updateApplication.mutate(
      { id: application.id, recipientEmail: val },
      {
        onSuccess: (data) => {
          if (data.error) {
            setEmailError(t('applications.detail.email.emailInvalid'));
          }
        },
      }
    );
  };

  if (docsLoading || (!!defaultResumeId && resumeQuery.isLoading)) {
    return (
      <div className="h-full overflow-y-auto px-6 py-5">
        <CardSkeleton />
      </div>
    );
  }

  return (
    <div className="@container flex h-full min-h-0 flex-col">
      {/* Toolbar: recipient inputs + generate button */}
      <div className="shrink-0 space-y-3 border-b border-[var(--border-soft)] px-6 py-4">
        <div className="grid gap-3 @md:grid-cols-2">
          <div className="flex flex-col gap-1">
            <label
              htmlFor="applyemail-recipient-name"
              className="text-xs font-medium text-foreground/70"
            >
              {t('applications.detail.email.recipientNameLabel')}
            </label>
            <Input
              id="applyemail-recipient-name"
              variant="default"
              placeholder={t('applications.detail.email.recipientNamePlaceholder')}
              value={recipientName}
              onChange={(e) => setRecipientName(e.target.value)}
              onBlur={persistName}
            />
          </div>
          <div className="flex flex-col gap-1">
            <label
              htmlFor="applyemail-recipient-email"
              className="text-xs font-medium text-foreground/70"
            >
              {t('applications.detail.email.recipientEmailLabel')}
            </label>
            <Input
              id="applyemail-recipient-email"
              type="email"
              variant="default"
              placeholder={t('applications.detail.email.recipientEmailPlaceholder')}
              value={recipientEmail}
              onChange={(e) => {
                setRecipientEmail(e.target.value);
                setEmailError(null);
              }}
              onBlur={(e) => persistEmail(e.target.value)}
            />
            {emailError && (
              <p className="text-fine-print text-destructive" role="alert">
                {emailError}
              </p>
            )}
          </div>
        </div>

        <Button
          variant="primary"
          disabled={!canGenerate}
          loading={isGenerating}
          onClick={() => void handleGenerate()}
          className="gap-1.5"
        >
          <Sparkles size={13} />
          {isGenerating
            ? t('applications.detail.email.generating')
            : hasDraft
              ? t('applications.detail.email.regenerate')
              : t('applications.detail.email.generate')}
        </Button>
      </div>

      {/* Preview area — aria-live so screen readers hear when generation finishes */}
      <div
        className="min-h-0 flex-1 overflow-y-auto px-6 py-4"
        aria-live="polite"
        aria-atomic="false"
      >
        {!canUse && (
          <EmptyState
            icon={Sparkles}
            title={t('applications.detail.email.needsModel')}
            className="py-12"
          />
        )}
        {canUse && !resume && !hasDraft && !isGenerating && (
          <EmptyState
            icon={FileText}
            title={t('applications.detail.email.needsResume')}
            className="py-12"
          />
        )}
        {canUse && !!resume && !jobDesc && !hasDraft && (
          <EmptyState
            icon={Briefcase}
            title={t('applications.detail.email.needsJob')}
            className="py-12"
          />
        )}
        {canUse && !!resume && !!jobDesc && !hasDraft && !isGenerating && !genError && (
          <EmptyState
            icon={Mail}
            title={t('applications.detail.email.empty')}
            description={t('applications.detail.email.emptyDesc')}
            className="py-12"
          />
        )}

        {genError && (
          <p className="text-fine-print text-destructive" role="alert">
            {genError}
          </p>
        )}

        {/* Skeleton during the first tokens — avoids a bare-cursor flash */}
        {isGenerating && !streamText && (
          <div className="space-y-3">
            <RowSkeleton />
            <RowSkeleton />
          </div>
        )}

        {(hasDraft || (isGenerating && !!streamText)) && (
          <div className="space-y-4">
            {(subject || isGenerating) && (
              <div className="rounded-md border border-[var(--border-soft)] bg-foreground/[0.02] px-4 py-2.5">
                <div className="flex items-center justify-between gap-2">
                  <span className="block text-xs font-semibold uppercase tracking-[0.14em] text-foreground/70">
                    {t('applications.detail.email.subjectLabel')}
                  </span>
                  {!isGenerating && subject && (
                    <div className="flex items-center gap-0.5">
                      {canRewrite && (
                        <Button
                          variant="ghost"
                          type="button"
                          // Keep the live selection alive through the click — a bare
                          // click would collapse it before onClick reads it.
                          onMouseDown={(e) => e.preventDefault()}
                          onClick={(e) => openRewrite('subject', e.currentTarget)}
                          title={t('applications.detail.email.rewrite')}
                          aria-label={t('applications.detail.email.rewriteSubjectAriaLabel')}
                          className="h-auto gap-1 px-1.5 py-0.5 text-[11px] text-brand-soft"
                        >
                          <Sparkles size={11} />
                          {t('applications.detail.email.rewrite')}
                        </Button>
                      )}
                      <Button
                        variant="unstyled"
                        type="button"
                        onClick={handleCopySubject}
                        title={
                          subjectCopied
                            ? t('applications.detail.email.copied')
                            : t('applications.detail.email.copySubject')
                        }
                        aria-label={t('applications.detail.email.copySubject')}
                        className="rounded p-0.5 text-foreground/30 transition-colors hover:text-foreground/70"
                      >
                        {subjectCopied ? <Check size={13} /> : <ClipboardCopy size={13} />}
                      </Button>
                    </div>
                  )}
                </div>
                {isGenerating ? (
                  <p className="mt-1 text-caption font-medium text-foreground/85">{subject}</p>
                ) : (
                  <p
                    ref={subjectRef}
                    className="mt-1 select-text whitespace-pre-wrap text-caption font-medium text-foreground/85"
                  >
                    {subject}
                  </p>
                )}
              </div>
            )}

            <div className="space-y-1.5">
              <div className="flex items-center justify-between gap-2">
                <span className="block text-xs font-semibold uppercase tracking-[0.14em] text-foreground/70">
                  {t('applications.detail.email.bodyLabel')}
                </span>
                {canRewrite && !isGenerating && body && (
                  <Button
                    variant="ghost"
                    type="button"
                    onMouseDown={(e) => e.preventDefault()}
                    onClick={(e) => openRewrite('body', e.currentTarget)}
                    title={t('applications.detail.email.rewrite')}
                    aria-label={t('applications.detail.email.rewriteBodyAriaLabel')}
                    className="h-auto gap-1 px-1.5 py-0.5 text-[11px] text-brand-soft"
                  >
                    <Sparkles size={11} />
                    {t('applications.detail.email.rewrite')}
                  </Button>
                )}
              </div>
              {isGenerating ? (
                <StreamingText text={body} isStreaming />
              ) : (
                <p
                  ref={bodyRef}
                  className="select-text whitespace-pre-wrap break-words text-sm leading-relaxed text-foreground/85"
                >
                  {body}
                </p>
              )}
            </div>

            {/* One rewrite popover for whichever field is frozen — it portals to
                document.body off `anchorEl`, so a single instance serves both. */}
            <AnimatePresence>
              {frozen && (
                <RewritePopover
                  target={frozen.target}
                  docType="email"
                  model={model}
                  locale={meta.targetLanguage}
                  anchorEl={frozen.anchorEl}
                  onAccept={acceptRewrite}
                  onClose={closeRewrite}
                />
              )}
            </AnimatePresence>

            {!isGenerating && hasDraft && (
              <div className="flex flex-wrap items-center gap-2 pt-1">
                <Button variant="glass" size="sm" onClick={handleCopy} className="gap-1.5">
                  <ClipboardCopy size={13} />
                  {copied
                    ? t('applications.detail.email.copied')
                    : t('applications.detail.email.copy')}
                </Button>
                {mailtoHref && (
                  <Button
                    variant="glass"
                    size="sm"
                    className="gap-1.5"
                    onClick={() => {
                      window.open(mailtoHref, '_blank');
                    }}
                  >
                    <Mail size={13} />
                    {t('applications.detail.email.openMailto')}
                  </Button>
                )}
              </div>
            )}
          </div>
        )}
      </div>
    </div>
  );
}
