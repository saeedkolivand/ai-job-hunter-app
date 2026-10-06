import { Briefcase, FileText, Mail, Sparkles } from 'lucide-react';
import { useNavigate } from '@tanstack/react-router';

import { useTranslation } from '@ajh/translations';
import { Button, EmptyState } from '@ajh/ui';

import { ROUTES } from '@/constants/routes';

interface Props {
  canUse: boolean;
  hasResume: boolean;
  hasJobDesc: boolean;
  hasDraft: boolean;
  isGenerating: boolean;
  genError: string | null;
  saveFailed: boolean;
}

/** The empty/prerequisite states plus the generation / persistence error lines. */
export function EmailStatusMessages({
  canUse,
  hasResume,
  hasJobDesc,
  hasDraft,
  isGenerating,
  genError,
  saveFailed,
}: Props) {
  const { t } = useTranslation();
  const navigate = useNavigate();
  return (
    <>
      {!canUse && (
        <EmptyState
          icon={Sparkles}
          title={t('applications.detail.email.needsModel')}
          className="py-12"
        />
      )}
      {canUse && !hasResume && !hasDraft && !isGenerating && (
        <EmptyState
          icon={FileText}
          title={t('applications.detail.email.needsResume')}
          className="py-12"
          action={
            <Button
              variant="primary"
              size="sm"
              onClick={() => void navigate({ to: ROUTES.RESUMES })}
              className="gap-1.5"
            >
              <FileText size={13} />
              {t('applications.detail.email.addResume')}
            </Button>
          }
        />
      )}
      {canUse && hasResume && !hasJobDesc && !hasDraft && (
        <EmptyState
          icon={Briefcase}
          title={t('applications.detail.email.needsJob')}
          className="py-12"
        />
      )}
      {canUse && hasResume && hasJobDesc && !hasDraft && !isGenerating && !genError && (
        <EmptyState
          icon={Mail}
          title={t('applications.detail.email.empty')}
          description={t('applications.detail.email.emptyDesc')}
          className="py-12"
        />
      )}

      {genError && (
        <p className="text-fine-print text-red-400" role="alert">
          {genError}
        </p>
      )}

      {/* role="alert" (not "status"): this is a data-loss warning, and it
          matches the genError/emailError siblings. The weaker "status" also
          conflicted with the ancestor aria-live region's aria-atomic. */}
      {saveFailed && (
        <p className="text-fine-print text-amber-400/80" role="alert">
          {t('applications.detail.email.saveFailed')}
        </p>
      )}
    </>
  );
}
