import { Search } from 'lucide-react';
import { useState } from 'react';

import type { HybridSearchRequest } from '@ajh/shared';
import { Button, EmptyState, GlassCard, Input } from '@ajh/ui';

import { PageHeader } from '@/components/layout/PageHeader';
import { PageTransition } from '@/components/layout/PageTransition';
import { useTranslation } from '@/lib/i18n';
import { useSearch } from '@/services';

interface JobPayload {
  title?: string;
  company?: string;
  location?: string;
  url?: string;
  source?: string;
}

export function SearchPage() {
  const { t } = useTranslation();
  const [text, setText] = useState('');
  const [request, setRequest] = useState<HybridSearchRequest | null>(null);
  const { data: hits = [], isFetching } = useSearch(request);

  const submit = (e: React.FormEvent) => {
    e.preventDefault();
    const query = text.trim();
    if (!query) return;
    setRequest({ query, collection: 'jobs', topK: 20, semanticWeight: 0.7 });
  };

  return (
    <PageTransition className="h-full overflow-y-auto px-10 py-10">
      <PageHeader
        title={t('nav.search')}
        subtitle="Hybrid search · semantic + keyword over your scraped jobs."
      />

      <form onSubmit={submit} className="mt-6 flex max-w-xl gap-2">
        <Input
          value={text}
          onChange={(e) => setText(e.target.value)}
          placeholder="Search scraped jobs (e.g. senior rust engineer remote)"
          className="flex-1"
        />
        <Button type="submit" variant="glass" disabled={!text.trim()} loading={isFetching}>
          <Search size={14} /> Search
        </Button>
      </form>

      <div className="mt-6 max-w-2xl">
        {request && !isFetching && hits.length === 0 && (
          <EmptyState
            icon={Search}
            title="No matches"
            description="No scraped jobs matched. Scrape some jobs first, then search."
          />
        )}

        <div className="space-y-2">
          {hits.map((hit) => {
            const job = hit.payload as JobPayload;
            return (
              <GlassCard key={hit.id} className="flex items-center justify-between gap-4">
                <div className="min-w-0">
                  <div className="truncate text-sm font-medium text-foreground">
                    {job.title ?? 'Untitled role'}
                  </div>
                  <div className="mt-0.5 truncate text-xs text-foreground/55">
                    {[job.company, job.location, job.source].filter(Boolean).join(' · ')}
                  </div>
                </div>
                <div className="shrink-0 text-right">
                  <div className="text-sm font-semibold text-brand">
                    {Math.round(hit.score * 100)}%
                  </div>
                  <div className="text-[10px] uppercase tracking-[0.2em] text-foreground/35">
                    match
                  </div>
                </div>
              </GlassCard>
            );
          })}
        </div>
      </div>
    </PageTransition>
  );
}
