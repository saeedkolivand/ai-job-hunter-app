import type { AtsView } from '@ajh/shared/ipc';
import { useTranslation } from '@ajh/translations';
import { Accordion } from '@ajh/ui';

import { useAtsView } from '@/services/use-ats-view';

import { AtsFieldRow } from './AtsFieldRow';

const CONTACT_FIELDS = ['name', 'email', 'phone', 'location'] as const;

function AtsViewBody({ view }: { view: AtsView }) {
  const { t } = useTranslation();
  return (
    <div className="space-y-3">
      <p className="text-[11px] text-foreground/65">{t('atsView.hint')}</p>
      <ul aria-label={t('atsView.groups.contact')}>
        {CONTACT_FIELDS.map((key) => (
          <AtsFieldRow key={key} label={t(`atsView.contact.${key}`)} field={view.contact[key]} />
        ))}
        <AtsFieldRow
          label={t('atsView.readingOrder')}
          field={view.readingOrder}
          foundText={t('atsView.readingOrderOk')}
          missingText={t('atsView.readingOrderUnchecked')}
        />
      </ul>
      <ul aria-label={t('atsView.groups.sections')}>
        {view.sections.map((section, i) => (
          <AtsFieldRow
            key={`${section.kind}-${section.heading ?? 'missing'}-${i}`}
            label={t(`atsView.section.${section.kind}`)}
            field={{ status: section.status, reason: section.reason }}
            value={section.heading}
          />
        ))}
      </ul>
      {view.roles.map((role, i) => (
        <ul key={i} aria-label={t('atsView.groups.role', { n: i + 1 })}>
          <AtsFieldRow label={t('atsView.role.title')} field={role.title} />
          <AtsFieldRow label={t('atsView.role.company')} field={role.company} />
          <AtsFieldRow label={t('atsView.role.start')} field={role.start} />
          <AtsFieldRow label={t('atsView.role.end')} field={role.end} />
        </ul>
      ))}
    </div>
  );
}

/**
 * "What an ATS sees" for an exported résumé file: a collapsed one-line summary
 * that expands to the per-field readout of a strict, rigid-parser reading.
 * Advisory only — a failed read (or no path yet) renders nothing, and nothing here blocks
 * an export.
 */
export function AtsViewPanel({ path }: { path: string | null }) {
  return path ? <LoadedAtsViewPanel path={path} /> : null;
}

function LoadedAtsViewPanel({ path }: { path: string }) {
  const { t } = useTranslation();
  const { data, isError } = useAtsView(path);
  // A failed re-read keeps the previous data in the query; never show it as current.
  if (!data || isError) return null;
  return (
    <Accordion
      title={t('atsView.title', { found: data.summary.found, total: data.summary.total })}
      content={<AtsViewBody view={data} />}
    />
  );
}
