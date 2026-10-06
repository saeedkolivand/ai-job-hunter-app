import { Check, ClipboardCopy, Mail, Sparkles } from 'lucide-react';
import { AnimatePresence } from 'motion/react';

import { useTranslation } from '@ajh/translations';
import { Button, StreamingText } from '@ajh/ui';

import { RewritePopover } from '@/components/generation/EditableOutput/RewritePopover';

import type { EmailRewrite } from './useEmailRewrite';

interface Props {
  subject: string;
  body: string;
  isGenerating: boolean;
  hasDraft: boolean;
  canRewrite: boolean;
  model: React.ComponentProps<typeof RewritePopover>['model'];
  locale: string;
  rewrite: EmailRewrite;
  copied: boolean;
  subjectCopied: boolean;
  onCopy: () => void;
  onCopySubject: () => void;
  mailtoHref: string | undefined;
}

const SECTION_LABEL = 'block text-xs font-semibold uppercase tracking-[0.14em] text-foreground/70';

/** The subject card, body, rewrite popover and copy / mailto actions of a draft. */
export function EmailDraftView({
  subject,
  body,
  isGenerating,
  hasDraft,
  canRewrite,
  model,
  locale,
  rewrite,
  copied,
  subjectCopied,
  onCopy,
  onCopySubject,
  mailtoHref,
}: Props) {
  const { t } = useTranslation();
  const { subjectRef, bodyRef, frozen, openRewrite, closeRewrite, acceptRewrite } = rewrite;

  return (
    <div className="space-y-4">
      {(subject || isGenerating) && (
        <div className="rounded-md border border-[var(--border-soft)] bg-foreground/[0.02] px-4 py-2.5">
          <div className="flex items-center justify-between gap-2">
            <span className={SECTION_LABEL}>{t('applications.detail.email.subjectLabel')}</span>
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
                  onClick={onCopySubject}
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
          <span className={SECTION_LABEL}>{t('applications.detail.email.bodyLabel')}</span>
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
            locale={locale}
            anchorEl={frozen.anchorEl}
            onAccept={acceptRewrite}
            onClose={closeRewrite}
          />
        )}
      </AnimatePresence>

      {!isGenerating && hasDraft && (
        <div className="flex flex-wrap items-center gap-2 pt-1">
          <Button variant="glass" size="sm" onClick={onCopy} className="gap-1.5">
            <ClipboardCopy size={13} />
            {copied ? t('applications.detail.email.copied') : t('applications.detail.email.copy')}
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
  );
}
