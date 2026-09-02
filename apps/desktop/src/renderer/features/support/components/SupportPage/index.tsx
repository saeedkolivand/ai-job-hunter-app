import { Search, SearchX } from 'lucide-react';
import { useState } from 'react';

import { TEST_IDS } from '@ajh/test-ids';
import { useTranslation } from '@ajh/translations';
import { Accordion, EmptyState, Input } from '@ajh/ui';

import { PageHeader } from '@/components/layout/PageHeader';
import { PageTransition } from '@/components/layout/PageTransition';
import { matchesHelpQuery } from '@/features/support/help-search';
import { getSupportSections } from '@/features/support/support-data';

export function SupportPage() {
  const { t } = useTranslation();
  const [query, setQuery] = useState('');

  // A whitespace-only query matches everything, so it must not count as
  // "searching" either — otherwise it would expand the whole page.
  const searching = query.trim() !== '';

  const sections = getSupportSections(t)
    .map((section) => ({
      ...section,
      problems: section.problems.filter((p) => matchesHelpQuery(query, `${p.q} ${p.a}`)),
    }))
    .filter((section) => section.problems.length > 0);

  return (
    <PageTransition className="h-full overflow-y-auto px-10 py-10">
      <div className="mx-auto max-w-3xl">
        <PageHeader
          title={t('support.faq.title')}
          subtitle={t('support.faq.subtitle')}
          badge={t('support.faq.badge')}
        />

        <Input
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          placeholder={t('support.search.placeholder')}
          // The placeholder is an instruction, not a name — a rotor needs the
          // short label instead.
          aria-label={t('support.search.ariaLabel')}
          prefix={<Search size={14} />}
          allowClear
          variant="default"
          data-testid={TEST_IDS.support.searchInput}
        />

        {sections.length === 0 ? (
          <div data-testid={TEST_IDS.support.emptyState}>
            <EmptyState
              icon={SearchX}
              title={t('support.search.noResultsTitle')}
              description={t('support.search.noResultsBody')}
            />
          </div>
        ) : (
          <div className="mt-6 space-y-8">
            {sections.map((section) => {
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
                    {section.problems.map((p) => (
                      // `defaultOpen` is read once on mount, so the key carries
                      // the searching flag: entering or clearing a query
                      // remounts the accordions with the right open state.
                      <Accordion
                        key={`${p.id}-${searching}`}
                        title={p.q}
                        content={p.a}
                        defaultOpen={searching}
                      />
                    ))}
                  </div>
                </div>
              );
            })}
          </div>
        )}
      </div>
    </PageTransition>
  );
}
