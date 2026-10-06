import { Check, Copy, Save, Wand2 } from 'lucide-react';

import { useTranslation } from '@ajh/translations';
import { Button, Input, StreamingText } from '@ajh/ui';

import { CONNECTION_NOTE_LIMIT } from '@/lib/generate';

import type { useReferralDraft } from './useReferralDraft';

/** Preset improve instructions — key matches i18n `improvePresets.*`. */
const IMPROVE_PRESETS = ['warmer', 'shorter', 'moreSpecific', 'fixGrammar'] as const;

interface ReferralDraftOutputProps {
  gen: ReturnType<typeof useReferralDraft>;
  isNote: boolean;
  overLimit: boolean;
  copied: boolean;
  onCopy: () => void;
  saved: boolean;
  saving: boolean;
  canSave: boolean;
  onSave: () => void;
  /** The free-text improve instruction lives with the modal so it survives the draft being cleared. */
  improveInstruction: string;
  onImproveInstructionChange: (v: string) => void;
  onImprove: (instruction: string) => void;
}

/** Draft output — generated message, Improve with AI affordance, and actions. */
export function ReferralDraftOutput({
  gen,
  isNote,
  overLimit,
  copied,
  onCopy,
  saved,
  saving,
  canSave,
  onSave,
  improveInstruction,
  onImproveInstructionChange,
  onImprove,
}: ReferralDraftOutputProps) {
  const { t } = useTranslation();
  return (
    <div className="surface-card space-y-1.5 rounded-lg px-3 py-2.5">
      <StreamingText text={gen.draft} isStreaming={gen.generating} />
      <div className="flex items-center justify-between gap-2 pt-1">
        {isNote ? (
          <span
            className={
              overLimit
                ? 'text-[10px] font-medium text-red-300/90'
                : 'text-[10px] text-foreground/40'
            }
          >
            {gen.draft.length}/{CONNECTION_NOTE_LIMIT}
            {overLimit ? ` · ${t('autopilot.referral.overLimit')}` : ''}
          </span>
        ) : (
          <span />
        )}
        <div className="flex items-center gap-2">
          <Button
            variant="glass"
            disabled={!gen.draft || overLimit || gen.generating}
            onClick={onCopy}
          >
            {copied ? <Check size={12} /> : <Copy size={12} />}
            {copied ? t('autopilot.referral.copied') : t('autopilot.referral.copy')}
          </Button>
          <Button variant="glass" loading={saving} disabled={!canSave || saving} onClick={onSave}>
            {saved ? <Check size={12} /> : <Save size={12} />}
            {saved ? t('autopilot.referral.saved') : t('autopilot.referral.save')}
          </Button>
        </div>
      </div>

      {/* Improve with AI — only visible when a draft exists and not streaming. */}
      {gen.draft && !gen.generating && (
        <div className="space-y-2 border-t border-[var(--border-clear)] pt-2">
          {/* Preset chips */}
          <div
            className="flex flex-wrap gap-1.5"
            role="group"
            aria-label={t('autopilot.referral.improveLabel')}
          >
            {IMPROVE_PRESETS.map((preset) => (
              <Button
                key={preset}
                variant="glass"
                disabled={!gen.canGenerate}
                onClick={() => onImprove(t(`autopilot.referral.improvePresets.${preset}`))}
                className="h-auto px-2 py-0.5 text-[10px]"
              >
                {t(`autopilot.referral.improvePresets.${preset}`)}
              </Button>
            ))}
          </div>

          {/* Free-text instruction */}
          <div className="flex gap-1.5">
            <Input
              id="referral-improve-instruction"
              variant="default"
              className="min-w-0 flex-1 shadow-none"
              value={improveInstruction}
              onChange={(e) => onImproveInstructionChange(e.target.value)}
              placeholder={t('autopilot.referral.improveInstructionPlaceholder')}
              aria-label={t('autopilot.referral.improveInstruction')}
              onKeyDown={(e) => {
                if (e.key === 'Enter' && !e.shiftKey) {
                  e.preventDefault();
                  onImprove(improveInstruction);
                }
              }}
            />
            <Button
              variant="glass"
              disabled={!gen.canGenerate || !improveInstruction.trim()}
              onClick={() => onImprove(improveInstruction)}
              aria-label={t('autopilot.referral.improveApply')}
              className="shrink-0"
            >
              <Wand2 size={12} />
              {t('autopilot.referral.improveApply')}
            </Button>
          </div>
        </div>
      )}
    </div>
  );
}
