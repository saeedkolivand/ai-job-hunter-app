import { RefreshCw, Settings2, UserPlus } from 'lucide-react';

import { useTranslation } from '@ajh/translations';
import { Button } from '@ajh/ui';

import type { GenerationMeta, TemplateId } from '@/lib/generate';

import { GenerationOutput } from './GenerationOutput';
import type { TailorTarget } from './useTailorGeneration';

interface Props {
  target: TailorTarget;
  jobDesc: string;
  // Output / doc state from useTailorGeneration.
  activeOut: 'resume' | 'cover';
  setActiveOut: (o: 'resume' | 'cover') => void;
  // Render-time template/ATS preference (sticky store) — drives the live preview
  // and the export. Picked on the results toolbar; never regenerates.
  templateId: TemplateId;
  atsMode: boolean;
  onTemplateChange: (id: TemplateId) => void;
  onAtsModeChange: (v: boolean) => void;
  output: string;
  onEdit: (text: string) => void;
  meta: GenerationMeta | null;
  copied: boolean;
  onCopy: () => void;
  exportOpen: boolean;
  setExportOpen: React.Dispatch<React.SetStateAction<boolean>>;
  onExport: (fmt: 'pdf' | 'docx' | 'txt') => void;
  // Actions.
  onRegenerate: () => void;
  onEditSettings: () => void;
  onReferral: () => void;
}

/**
 * Done stage: the tailored documents (resume/cover/job-ad tabs via
 * {@link GenerationOutput}), a referral action, and the regenerate / edit-settings
 * footer. The application-questions assistant now lives in a header-triggered
 * modal ({@link ApplicationQuestionsModal}), so it no longer crowds this stack.
 */
export function ResultsPanel({
  target,
  jobDesc,
  activeOut,
  setActiveOut,
  templateId,
  atsMode,
  onTemplateChange,
  onAtsModeChange,
  output,
  onEdit,
  meta,
  copied,
  onCopy,
  exportOpen,
  setExportOpen,
  onExport,
  onRegenerate,
  onEditSettings,
  onReferral,
}: Props) {
  const { t } = useTranslation();

  return (
    <div className="flex h-full min-h-0 flex-col">
      <div className="flex min-h-0 flex-1 flex-col gap-3 overflow-y-auto px-8 py-6">
        <GenerationOutput
          target={target}
          activeOut={activeOut}
          setActiveOut={setActiveOut}
          templateId={templateId}
          atsMode={atsMode}
          onTemplateChange={onTemplateChange}
          onAtsModeChange={onAtsModeChange}
          output={output}
          onEdit={onEdit}
          editable
          meta={meta}
          copied={copied}
          onCopy={onCopy}
          exportOpen={exportOpen}
          setExportOpen={setExportOpen}
          onExport={onExport}
          jobDesc={jobDesc}
        />

        <Button
          variant="ghost"
          onClick={onReferral}
          className="self-start gap-1.5 text-foreground/50 hover:text-foreground/80"
        >
          <UserPlus size={13} /> {t('autopilot.referral.open')}
        </Button>
      </div>

      <div className="flex shrink-0 items-center justify-between border-t border-white/[0.06] px-8 py-4">
        <Button
          variant="ghost"
          onClick={onEditSettings}
          className="gap-1.5 text-foreground/50 hover:text-foreground/80"
        >
          <Settings2 size={13} /> {t('autopilot.apply.wizard.results.edit')}
        </Button>
        <Button variant="glass" onClick={onRegenerate} className="gap-1.5">
          <RefreshCw size={13} /> {t('autopilot.apply.wizard.results.regenerate')}
        </Button>
      </div>
    </div>
  );
}
