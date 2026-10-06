import { Banknote, CalendarClock, type LucideIcon, StickyNote, UserRound } from 'lucide-react';

import { useTranslation } from '@ajh/translations';
import { cn, IconBadge, Input, SectionLabel, TextArea } from '@ajh/ui';

import type { nextActionLabel } from '@/features/applications/lib/stale';

import { FieldError, FieldLabel } from './DetailChrome';
import type { OverviewFields } from './useOverviewFields';

/**
 * A flat Overview section on the white detail sheet: an {@link IconBadge} +
 * {@link SectionLabel} header over its fields, separated from the previous
 * section by a hairline (none above the first). Replaces the old nested cards.
 */
function OverviewSection({
  icon,
  label,
  children,
}: {
  icon: LucideIcon;
  label: string;
  children: React.ReactNode;
}) {
  return (
    <section className="space-y-3 border-t border-[var(--border-soft)] py-5 first:border-t-0">
      <div className="flex items-center gap-2">
        <IconBadge icon={icon} size="sm" />
        <SectionLabel>{label}</SectionLabel>
      </div>
      {children}
    </section>
  );
}

/** Label + control (+ optional trailing error/hint) in the overview field grid. */
function Field({ id, label, children }: { id: string; label: string; children: React.ReactNode }) {
  return (
    <div className="flex flex-col gap-1.5">
      <FieldLabel htmlFor={id}>{label}</FieldLabel>
      {children}
    </div>
  );
}

/** The value / change / blur props every save-on-blur overview input shares. */
const bind = (f: { value: string; onChange: (v: string) => void; onBlur: () => void }) => ({
  value: f.value,
  onChange: (e: { target: { value: string } }) => f.onChange(e.target.value),
  onBlur: f.onBlur,
});

export function OverviewTab({
  fields,
  nextState,
}: {
  fields: OverviewFields;
  nextState: ReturnType<typeof nextActionLabel>;
}) {
  const { t } = useTranslation();
  const { nextAction, notes, contactName, contactEmail, comp } = fields;

  return (
    <div className="@container h-full overflow-y-auto px-6">
      {/* Follow-up leads the sheet: the one field that drives the
          backend reminder sweep, so it must not sit last. */}
      <OverviewSection icon={CalendarClock} label={t('applications.detail.followUpSection')}>
        <div className="grid gap-4 @md:grid-cols-2">
          <Field id="appdetail-next-action" label={t('applications.detail.nextActionLabel')}>
            <Input
              id="appdetail-next-action"
              variant="default"
              type="date"
              {...bind(nextAction)}
              className="w-full"
            />
            <p
              className={cn(
                'text-fine-print',
                nextState === 'overdue' ? 'text-red-400' : 'text-foreground/70'
              )}
            >
              {nextState === 'overdue'
                ? t('applications.detail.followUpOverdueHint')
                : nextState === 'upcoming'
                  ? t('applications.detail.followUpUpcomingHint')
                  : t('applications.detail.followUpNoneHint')}
            </p>
          </Field>
        </div>
      </OverviewSection>

      <OverviewSection icon={StickyNote} label={t('applications.detail.notesLabel')}>
        <label htmlFor="appdetail-notes" className="sr-only">
          {t('applications.detail.notesLabel')}
        </label>
        <TextArea
          id="appdetail-notes"
          variant="glass"
          rows={4}
          className="!shadow-none"
          placeholder={t('applications.detail.notesPlaceholder')}
          {...bind(notes)}
        />
      </OverviewSection>

      <OverviewSection icon={UserRound} label={t('applications.detail.contactSection')}>
        <div className="grid gap-4 @md:grid-cols-2">
          <Field id="appdetail-contact-name" label={t('applications.detail.contactNameLabel')}>
            <Input
              id="appdetail-contact-name"
              variant="default"
              placeholder={t('applications.detail.contactNamePlaceholder')}
              {...bind(contactName)}
            />
            {contactName.error && (
              <FieldError>{t('applications.detail.contactSaveError')}</FieldError>
            )}
          </Field>

          <Field id="appdetail-contact-email" label={t('applications.detail.contactEmailLabel')}>
            <Input
              id="appdetail-contact-email"
              variant="default"
              type="email"
              placeholder={t('applications.detail.contactEmailPlaceholder')}
              {...bind(contactEmail)}
            />
            {contactEmail.error && (
              <FieldError>{t('applications.detail.email.emailInvalid')}</FieldError>
            )}
          </Field>
        </div>
      </OverviewSection>

      <OverviewSection icon={Banknote} label={t('applications.detail.compensationSection')}>
        <div className="grid gap-4 @md:grid-cols-2">
          <Field id="appdetail-comp" label={t('applications.detail.compLabel')}>
            <Input
              id="appdetail-comp"
              variant="default"
              placeholder={t('applications.detail.compPlaceholder')}
              {...bind(comp)}
            />
          </Field>
        </div>
      </OverviewSection>
    </div>
  );
}
