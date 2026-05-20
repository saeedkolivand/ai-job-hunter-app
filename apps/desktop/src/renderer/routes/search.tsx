import { createFileRoute } from '@tanstack/react-router';
import { useTranslation } from '@/lib/i18n';
import { PageHeader } from '@/components/layout/PageHeader';
import { PageTransition } from '@/components/layout/PageTransition';

export const Route = createFileRoute('/search')({ component: SearchPage });

function SearchPage() {
  const { t } = useTranslation();
  return (
    <PageTransition className="h-full overflow-y-auto px-10 py-10">
      <PageHeader
        title={t('nav.search')}
        subtitle="Hybrid search · semantic + keyword + metadata + recency."
      />
    </PageTransition>
  );
}
