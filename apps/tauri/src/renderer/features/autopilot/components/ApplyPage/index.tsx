import { ArrowLeft, HelpCircle, UserPlus, Wand2 } from 'lucide-react';
import { useState } from 'react';

import type { AutopilotFoundJob } from '@ajh/shared';
import { useTranslation } from '@ajh/translations';
import { Button } from '@ajh/ui';

import { scoreToLevel } from '@/features/autopilot/lib/match-level';
import {
  TailorFlow,
  type TailorFlowController,
  type TailorFlowPersistence,
} from '@/features/documents/components/TailorFlow';
import { useSessionStore } from '@/store/session-store';

interface Props {
  job: AutopilotFoundJob;
  resumeText?: string;
  board: string;
  onBack: () => void;
}

/**
 * Thin host for the autopilot tailoring flow: a slim page header + the shared
 * {@link TailorFlow} body. The wizard/template/ATS state is persisted on the
 * `autopilot` session slice (this surface owns it); TailorFlow surfaces a
 * controller so the header can read the derived stage and trigger its modals.
 */
export function ApplyPage({ job, resumeText, board, onBack }: Props) {
  const { t } = useTranslation();
  const { autopilot, setAutopilot } = useSessionStore();
  const [controller, setController] = useState<TailorFlowController | null>(null);

  const persistence: TailorFlowPersistence = {
    wizardStep: autopilot.applyWizardStep,
    wizardForm: autopilot.applyWizardForm,
    templateId: autopilot.applyTemplateId,
    atsMode: autopilot.applyAtsMode,
    setWizardStep: (v) => setAutopilot({ applyWizardStep: v }),
    setWizardForm: (v) => setAutopilot({ applyWizardForm: v }),
    setTemplateId: (v) => setAutopilot({ applyTemplateId: v }),
    setAtsMode: (v) => setAutopilot({ applyAtsMode: v }),
  };

  return (
    <div className="flex h-full flex-col">
      {/* Slim page header (persists across all stages) */}
      <div className="flex shrink-0 items-center gap-3 border-b border-white/[0.06] px-8 py-4">
        <Button
          onClick={onBack}
          variant="ghost"
          className="shrink-0 gap-1.5 text-foreground/50 hover:text-foreground/80"
        >
          <ArrowLeft size={14} /> {t('autopilot.apply.back')}
        </Button>
        <div className="min-w-0 flex-1">
          <div className="flex items-center gap-2">
            <Wand2 size={14} className="shrink-0 text-brand-soft" />
            <span className="truncate text-base font-semibold text-foreground/90">{job.title}</span>
            {typeof job.score === 'number' && (
              <span className="shrink-0 rounded-full bg-brand/10 px-2 py-0.5 text-[10px] font-medium text-brand-soft">
                {t(`autopilot.wizard.filter.matchLevel.${scoreToLevel(job.score)}`)}{' '}
                {t('autopilot.apply.match')}
              </span>
            )}
          </div>
          <div className="truncate text-[11px] text-foreground/40">
            {job.company}
            {job.location ? ` · ${job.location}` : ''}
          </div>
        </div>
        {controller?.stage === 'done' && (
          <Button
            variant="glass"
            onClick={() => controller.openQuestions()}
            className="shrink-0 gap-1.5 text-brand-soft"
          >
            <HelpCircle size={13} /> {t('autopilot.apply.questions.title')}
            {controller.questionsCount > 0 && (
              <span className="rounded-full bg-brand/15 px-1.5 py-0.5 text-[9px] text-brand-soft">
                {controller.questionsCount}
              </span>
            )}
          </Button>
        )}
        <Button
          variant="glass"
          disabled={!controller}
          onClick={() => controller?.openReferral()}
          className="shrink-0 gap-1.5 text-brand-soft"
        >
          <UserPlus size={13} /> {t('autopilot.referral.open')}
        </Button>
      </div>

      {/* Shared tailoring body */}
      <div className="min-h-0 flex-1">
        <TailorFlow
          job={job}
          resumeText={resumeText}
          board={board}
          contextId={`autopilot:${job.url}`}
          jobUrl={job.url}
          persistence={persistence}
          onController={setController}
        />
      </div>
    </div>
  );
}
