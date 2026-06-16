import { UserCheck } from 'lucide-react';
import { useState } from 'react';

import type { ContactFieldConflict, ContactProfile } from '@ajh/shared';
import { useTranslation } from '@ajh/translations';
import { Button, Input, ModalShell, SegmentedControl, useNotification } from '@ajh/ui';

import { useContactProfile, useSaveContactProfile } from '@/services';

interface ContactConflictModalProps {
  open: boolean;
  conflicts: ContactFieldConflict[];
  /** Dismiss without changing the saved profile (Escape / backdrop / Cancel). */
  onClose: () => void;
  /** Fired after the merged profile is persisted. */
  onResolved?: () => void;
}

/** Plain string properties on `ContactProfile` that a conflict can target. */
type StringProfileField = 'fullName' | 'email' | 'phone' | 'linkedin' | 'github' | 'website';

const STRING_FIELDS: readonly StringProfileField[] = [
  'fullName',
  'email',
  'phone',
  'linkedin',
  'github',
  'website',
];

/** Per-field resolution choice. `mine` keeps the saved value (non-destructive default). */
type Choice = 'mine' | 'resume';

interface FieldState {
  choice: Choice;
  /** The editable value, seeded from the chosen side and tweakable before saving. */
  value: string;
}

/** Maps a stable conflict `field` key to the i18n label key for that field. */
function labelKey(field: string): string {
  // Identity fields reuse the existing contact-profile labels where possible.
  switch (field) {
    case 'fullName':
      return 'settings.contactProfile.fullName';
    case 'email':
      return 'settings.contactProfile.email';
    case 'phone':
      return 'settings.contactProfile.phone';
    case 'location':
      return 'settings.contactProfile.location';
    case 'linkedin':
      return 'settings.contactProfile.linkedin';
    case 'github':
      return 'settings.contactProfile.github';
    case 'website':
      return 'settings.contactProfile.website';
    default:
      return field;
  }
}

/**
 * Post-import follow-up that lets the user reconcile saved contact fields with
 * the values extracted from a freshly-imported résumé. It is never a gate — the
 * résumé is already imported and empty fields were already autofilled silently.
 * Dismissing keeps the current profile untouched.
 *
 * Per conflict the user picks "keep mine" or "use résumé's", and can edit the
 * resulting value inline before saving. On save we read the current profile as
 * the base and apply each chosen value onto the matching `ContactProfile`
 * property (string props directly; `location` → `location.default`, preserving
 * any existing `byLang`).
 */
export function ContactConflictModal({
  open,
  conflicts,
  onClose,
  onResolved,
}: ContactConflictModalProps) {
  const { t } = useTranslation();
  const notify = useNotification();
  const { data: profile } = useContactProfile();
  const { mutateAsync: save, isPending } = useSaveContactProfile();

  // Seed one row per conflict — default to keeping the saved value (the
  // non-destructive choice), with the editable value primed to that same side.
  const seed = (list: ContactFieldConflict[]): Record<string, FieldState> => {
    const next: Record<string, FieldState> = {};
    for (const c of list) next[c.field] = { choice: 'mine', value: c.current };
    return next;
  };

  // Seeds once per mount. Call sites remount the modal (via a `key` that changes
  // per import) so a fresh conflict set always re-runs this initializer cleanly —
  // no render-phase re-seeding needed.
  const [fields, setFields] = useState<Record<string, FieldState>>(() => seed(conflicts));

  const setChoice = (field: string, choice: Choice) =>
    setFields((prev) => {
      const conflict = conflicts.find((c) => c.field === field);
      if (!conflict) return prev;
      // Switching sides re-seeds the editable value from the picked side.
      const value = choice === 'mine' ? conflict.current : conflict.suggested;
      return { ...prev, [field]: { choice, value } };
    });

  const setValue = (field: string, value: string) =>
    setFields((prev) => ({
      ...prev,
      [field]: { choice: prev[field]?.choice ?? 'mine', value },
    }));

  const handleSave = async () => {
    const base: ContactProfile = { ...(profile ?? {}) };
    for (const c of conflicts) {
      const value = (fields[c.field]?.value ?? c.current).trim();
      if (STRING_FIELDS.includes(c.field as StringProfileField)) {
        base[c.field as StringProfileField] = value || undefined;
      } else if (c.field === 'location' && value) {
        // Preserve any per-language overrides; only the default value changes.
        // An emptied value means "keep current" — leave `location` untouched so
        // an existing `byLang` and a real `default` survive.
        base.location = { ...base.location, default: value };
      }
    }
    try {
      await save(base);
      notify.success({ message: t('settings.contactConflict.saved') });
      onResolved?.();
      onClose();
    } catch {
      notify.error({ message: t('settings.contactConflict.saveFailed') });
    }
  };

  const options = [
    { value: 'mine' as const, label: t('settings.contactConflict.keepMine') },
    { value: 'resume' as const, label: t('settings.contactConflict.useResume') },
  ] as const;

  return (
    <ModalShell open={open} onClose={onClose} maxWidth="max-w-2xl">
      <div className="flex max-h-[85vh] flex-col">
        <div className="flex items-start gap-2 border-b border-foreground/10 px-6 py-5">
          <UserCheck size={16} className="mt-0.5 shrink-0 text-brand-soft" />
          <div className="flex flex-col gap-1">
            <span className="text-sm font-semibold text-foreground/85">
              {t('settings.contactConflict.title')}
            </span>
            <p className="text-xs text-foreground/55">
              {t('settings.contactConflict.description')}
            </p>
          </div>
        </div>

        <div className="flex flex-col gap-4 overflow-y-auto px-6 py-5">
          {conflicts.map((c) => {
            const state = fields[c.field] ?? { choice: 'mine' as Choice, value: c.current };
            return (
              <div
                key={c.field}
                className="flex flex-col gap-3 rounded-xl border border-foreground/10 bg-foreground/[0.03] p-4"
              >
                <div className="flex items-center justify-between gap-3">
                  <span className="text-xs font-medium text-foreground/70">
                    {t(labelKey(c.field))}
                  </span>
                  <SegmentedControl
                    options={options}
                    value={state.choice}
                    onChange={(choice) => setChoice(c.field, choice)}
                    ariaLabel={t('settings.contactConflict.choiceAria', {
                      field: t(labelKey(c.field)),
                    })}
                  />
                </div>

                <div className="grid grid-cols-1 gap-2 sm:grid-cols-2">
                  <div className="flex flex-col gap-1">
                    <span className="text-[10px] uppercase tracking-wider text-foreground/40">
                      {t('settings.contactConflict.savedLabel')}
                    </span>
                    <span className="truncate text-xs text-foreground/60">
                      {c.current || t('settings.contactConflict.emptyValue')}
                    </span>
                  </div>
                  <div className="flex flex-col gap-1">
                    <span className="text-[10px] uppercase tracking-wider text-foreground/40">
                      {t('settings.contactConflict.resumeLabel')}
                    </span>
                    <span className="truncate text-xs text-foreground/60">
                      {c.suggested || t('settings.contactConflict.emptyValue')}
                    </span>
                  </div>
                </div>

                <Input
                  aria-label={t('settings.contactConflict.editAria', {
                    field: t(labelKey(c.field)),
                  })}
                  value={state.value}
                  onChange={(e) => setValue(c.field, e.target.value)}
                />
              </div>
            );
          })}
        </div>

        <div className="flex justify-end gap-2 border-t border-foreground/10 px-6 py-4">
          <Button variant="ghost" onClick={onClose}>
            {t('settings.contactConflict.dismiss')}
          </Button>
          <Button onClick={() => void handleSave()} disabled={isPending}>
            {t('settings.contactConflict.save')}
          </Button>
        </div>
      </div>
    </ModalShell>
  );
}
