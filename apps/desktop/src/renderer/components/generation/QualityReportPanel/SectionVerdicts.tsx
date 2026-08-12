import { AlertTriangle, CheckCircle2, Wand2 } from 'lucide-react';
import { useState } from 'react';

import type { PipelineSectionKey } from '@ajh/shared';
import { useTranslation } from '@ajh/translations';
import { Button, TextArea } from '@ajh/ui';

import type { SectionVerdict } from '@/lib/generate';

export interface SectionVerdictsProps {
  sections: SectionVerdict[];
  /** Re-generate one section through the repair splice primitive. Omit to
   *  render verdicts read-only (an older run, or a surface with no run id). */
  onFixSection?: (sectionKey: PipelineSectionKey, note: string) => void;
  /** The section currently being regenerated — its row shows the busy state
   *  and every other Fix button is disabled (one splice at a time). */
  fixingSection?: string | null;
  /** A refusal the user must SEE: a non-latest run, a full admission bucket, a
   *  section the model returned truncated. Never auto-retried. */
  fixError?: string | null;
  /** Run-level repair summary — see the note below on why it isn't per-section. */
  repairRounds?: number;
  repairReverted?: boolean;
}

/**
 * Per-section verdict chips for a finished staged run, each with an optional
 * "Fix this section".
 *
 * Two honesty constraints are visible in the markup:
 *
 * - A section whose heading this build can't map to the closed
 *   `PipelineSectionKey` grammar shows its verdict but NO Fix button — sending
 *   a guessed key would be rejected at the boundary anyway, and guessing
 *   `"header"` in particular is what ADR-0021 forbids.
 * - "Auto-repaired" is reported once, for the RUN, not per section: the repair
 *   stage's artifact carries counts only (ADR-027 keeps section text out of it),
 *   so a per-section "repaired" chip would be invented.
 *
 * **Max depth does NOT change that second one, contrary to what this file's
 * earlier note anticipated.** Phase 4 added per-section artifacts for
 * GENERATION — `sections` emits `{section, sections, produced}` per section —
 * but repair is unchanged at both depths: one `{rounds, reverted,
 * truncatedAttempts, failedAttempts, budgeted, timedOut, criticalsRemaining}`
 * record for the whole stage, with nothing naming a section. Which section a
 * repair round touched is therefore still not recorded anywhere the renderer
 * can read, so the run-level line below stays the honest answer for both
 * depths. The per-section verdicts that DO exist are computed from the stored
 * report (`buildSectionVerdicts`), which is what the rows above show.
 */
export function SectionVerdicts({
  sections,
  onFixSection,
  fixingSection,
  fixError,
  repairRounds,
  repairReverted,
}: SectionVerdictsProps) {
  const { t } = useTranslation();
  const [openNoteFor, setOpenNoteFor] = useState<string | null>(null);
  const [note, setNote] = useState('');

  if (sections.length === 0) return null;

  const busy = !!fixingSection;

  const submit = (sectionKey: PipelineSectionKey) => {
    onFixSection?.(sectionKey, note.trim());
    setOpenNoteFor(null);
    setNote('');
  };

  return (
    <div className="border-t border-white/[0.06] pt-4">
      <h3 className="mb-2 text-[11px] font-semibold uppercase tracking-[0.14em] text-foreground/45">
        {t('quality.panel.sections.title')}
      </h3>

      <ul className="space-y-1.5">
        {sections.map((section) => {
          const clean = section.issues === 0;
          const fixable = !!onFixSection && section.sectionKey !== null;
          const fixing = fixingSection === section.sectionKey;
          const noteOpen = openNoteFor === section.sectionKey;
          return (
            <li
              // Labels are the document's own heading text now, and one line
              // can name two canonical sections ("Skills and Experience"), so
              // the label alone is no longer a unique React key.
              key={`${section.sectionKey ?? 'unmapped'}::${section.label}`}
              className="rounded-lg border border-white/[0.06] bg-white/[0.02] px-3 py-2"
            >
              <div className="flex flex-wrap items-center gap-2">
                <span className="flex min-w-0 items-center gap-1.5 text-[11px] font-medium text-foreground/80">
                  {clean ? (
                    <CheckCircle2 size={11} className="shrink-0 text-emerald-400" />
                  ) : (
                    <AlertTriangle size={11} className="shrink-0 text-amber-400" />
                  )}
                  <span className="truncate">{section.label}</span>
                </span>
                <span className="text-[10px] text-foreground/45">
                  {clean
                    ? t('quality.panel.sections.clean')
                    : t('quality.panel.sections.issues', { count: section.issues })}
                  {section.criticals > 0 &&
                    ` · ${t('quality.panel.sections.criticals', { count: section.criticals })}`}
                </span>
                <span className="flex-1" />
                {fixable ? (
                  <Button
                    type="button"
                    variant="ghost"
                    size="sm"
                    disabled={busy}
                    onClick={() => setOpenNoteFor(noteOpen ? null : section.sectionKey)}
                    className="shrink-0 text-[10px]"
                  >
                    <Wand2 size={10} className={fixing ? 'animate-pulse' : undefined} />
                    {fixing ? t('quality.panel.sections.fixing') : t('quality.panel.sections.fix')}
                  </Button>
                ) : (
                  onFixSection && (
                    <span className="shrink-0 text-[10px] text-foreground/30">
                      {t('quality.panel.sections.notAddressable')}
                    </span>
                  )
                )}
              </div>

              {noteOpen && section.sectionKey && (
                <div className="mt-2 space-y-1.5">
                  <label
                    htmlFor={`fix-note-${section.sectionKey}`}
                    className="block text-[10px] text-foreground/45"
                  >
                    {t('quality.panel.sections.noteLabel')}
                  </label>
                  <TextArea
                    id={`fix-note-${section.sectionKey}`}
                    value={note}
                    onChange={(e) => setNote(e.target.value)}
                    maxLength={500}
                    rows={2}
                    placeholder={t('quality.panel.sections.notePlaceholder')}
                    className="text-[11px]"
                  />
                  <Button
                    type="button"
                    variant="primary"
                    size="sm"
                    disabled={busy}
                    onClick={() => submit(section.sectionKey as PipelineSectionKey)}
                  >
                    {t('quality.panel.sections.fixConfirm')}
                  </Button>
                </div>
              )}
            </li>
          );
        })}
      </ul>

      {fixError && (
        <p role="alert" className="mt-2 text-[10px] leading-relaxed text-red-400">
          {fixError}
        </p>
      )}

      {!!repairRounds && repairRounds > 0 && (
        <p className="mt-2 text-[10px] leading-relaxed text-foreground/40">
          {t('quality.panel.sections.repaired', { count: repairRounds })}{' '}
          {repairReverted && t('quality.panel.sections.reverted')}{' '}
          {t('quality.panel.sections.perSectionNote')}
        </p>
      )}
    </div>
  );
}
