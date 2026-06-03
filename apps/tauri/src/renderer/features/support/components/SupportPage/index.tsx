import { Accordion } from '@ajh/ui';

import { PageHeader } from '@/components/layout/PageHeader';
import { PageTransition } from '@/components/layout/PageTransition';
import { getSupportSections } from '@/features/support/support-data';
import { useTranslation } from '@/lib/i18n';

export function SupportPage() {
  const { t } = useTranslation();
  const SECTIONS = getSupportSections(t);

  return (
    <PageTransition className="h-full overflow-y-auto px-10 py-10">
      <div className="mx-auto max-w-3xl">
        <PageHeader
          title={t('support.faq.title')}
          subtitle={t('support.faq.subtitle')}
          badge={t('support.faq.badge')}
        />

        <div className="mt-2 space-y-8">
          {SECTIONS.map((section) => {
            const Icon = section.icon;
            return (
              <div key={section.label}>
                <div className="mb-3 flex items-center gap-2.5">
                  <div
                    className="flex h-7 w-7 items-center justify-center rounded-lg"
                    style={{ background: section.glow }}
                  >
                    <Icon size={14} className={section.color} />
                  </div>
                  <span className="text-xs font-semibold uppercase tracking-[0.18em] text-foreground/55">
                    {section.label}
                  </span>
                </div>

                <div className="space-y-2">
                  {section.problems.map((p, i) => (
                    <Accordion key={i} title={p.q} content={p.a} />
                  ))}
                </div>
              </div>
            );
          })}
        </div>
      </div>
    </PageTransition>
  );
}
