import { ClipboardCopy, Mail, Sparkles } from 'lucide-react';
import { useEffect, useRef, useState } from 'react';

import type { AiGenerationRecord, Application } from '@ajh/shared';
import { useTranslation } from '@ajh/translations';
import { Button, CardSkeleton, EmptyState, Input, RowSkeleton, StreamingText } from '@ajh/ui';

import { useCanUseAI, useSelectedModel } from '@/components/ui/ModelSelector';
import { useDefaultResumeId } from '@/hooks/useDefaultResumeId';
import { generateApplicationEmail, type GenerationMeta } from '@/lib/generate';
import { useDocuments, useDocumentText, useUpdateApplication } from '@/services';

import { extractRecipient } from '../../lib/extract-recipient';

interface Props {
  application: Application;
  matchingGenerations: AiGenerationRecord[];
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

  const docsQuery = useDocuments();
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

  // Abort any in-flight stream on unmount to prevent quota burn on tab change.
  useEffect(
    () => () => {
      abortRef.current?.abort();
    },
    []
  );

  const { subject, body } = splitEmail(streamText);
  const hasDraft = streamText.trim().length > 0;
  const canGenerate = canUse && !!model && !!resume && !!jobDesc && !isGenerating;

  const handleGenerate = async () => {
    if (!canGenerate) return;
    abortRef.current?.abort();
    abortRef.current = new AbortController();
    setIsGenerating(true);
    setStreamText('');
    setGenError(null);
    try {
      await generateApplicationEmail({
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

  if (docsQuery.isLoading || (!!defaultResumeId && resumeQuery.isLoading)) {
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
          <EmptyState title={t('applications.detail.email.needsModel')} className="py-12" />
        )}
        {canUse && !resume && !hasDraft && !isGenerating && (
          <EmptyState title={t('applications.detail.email.needsResume')} className="py-12" />
        )}
        {canUse && !!resume && !jobDesc && !hasDraft && (
          <EmptyState title={t('applications.detail.email.needsJob')} className="py-12" />
        )}
        {canUse && !!resume && !!jobDesc && !hasDraft && !isGenerating && !genError && (
          <EmptyState
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
                <span className="block text-xs font-semibold uppercase tracking-[0.14em] text-foreground/70">
                  {t('applications.detail.email.subjectLabel')}
                </span>
                <p className="mt-1 text-caption font-medium text-foreground/85">{subject}</p>
              </div>
            )}

            <div className="space-y-1.5">
              <span className="block text-xs font-semibold uppercase tracking-[0.14em] text-foreground/70">
                {t('applications.detail.email.bodyLabel')}
              </span>
              <StreamingText text={body} isStreaming={isGenerating} />
            </div>

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
