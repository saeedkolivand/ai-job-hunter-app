import { Search } from 'lucide-react';
import { useState } from 'react';

import { useTranslation } from '@ajh/translations';
import { Button, cn, EmptyState, Input } from '@ajh/ui';

import { KBArticleCard } from '../KBArticleCard';

const CATEGORIES = [
  { id: 'all', nameKey: 'support.knowledgeBase.all' },
  { id: 'ai', nameKey: 'support.knowledgeBase.aiModels' },
  { id: 'ollama', nameKey: 'support.knowledgeBase.ollama' },
  { id: 'documents', nameKey: 'support.knowledgeBase.documents' },
  { id: 'ocr', nameKey: 'support.knowledgeBase.ocr' },
  { id: 'scraping', nameKey: 'support.knowledgeBase.scraping' },
  { id: 'search', nameKey: 'support.knowledgeBase.search' },
  { id: 'performance', nameKey: 'support.knowledgeBase.performance' },
  { id: 'installation', nameKey: 'support.knowledgeBase.installation' },
] as const;

export function KnowledgeBase() {
  const { t } = useTranslation();
  const [searchQuery, setSearchQuery] = useState('');
  const [selectedCategory, setSelectedCategory] = useState('all');

  const ARTICLES = [
    {
      id: 1,
      category: 'ollama',
      title: t('support.knowledgeBase.article1Title'),
      summary: t('support.knowledgeBase.article1Summary'),
    },
    {
      id: 2,
      category: 'ai',
      title: t('support.knowledgeBase.article2Title'),
      summary: t('support.knowledgeBase.article2Summary'),
    },
    {
      id: 3,
      category: 'ocr',
      title: t('support.knowledgeBase.article3Title'),
      summary: t('support.knowledgeBase.article3Summary'),
    },
    {
      id: 4,
      category: 'scraping',
      title: t('support.knowledgeBase.article4Title'),
      summary: t('support.knowledgeBase.article4Summary'),
    },
    {
      id: 5,
      category: 'search',
      title: t('support.knowledgeBase.article5Title'),
      summary: t('support.knowledgeBase.article5Summary'),
    },
    {
      id: 6,
      category: 'performance',
      title: t('support.knowledgeBase.article6Title'),
      summary: t('support.knowledgeBase.article6Summary'),
    },
  ];

  const filteredArticles = ARTICLES.filter((a) => {
    const matchesCategory = selectedCategory === 'all' || a.category === selectedCategory;
    const q = searchQuery.toLowerCase();
    const matchesSearch =
      !q || a.title.toLowerCase().includes(q) || a.summary.toLowerCase().includes(q);
    return matchesCategory && matchesSearch;
  });

  return (
    <div className="space-y-6">
      <div className="glass-card rounded-2xl p-6">
        <h2 className="mb-4 text-lg font-semibold">
          {t('support.knowledgeBase.searchKnowledgeBase')}
        </h2>
        <Input
          type="search"
          value={searchQuery}
          onChange={(e) => setSearchQuery(e.target.value)}
          placeholder={t('support.knowledgeBase.searchPlaceholder')}
          className="w-full"
        />
      </div>

      <div className="glass-card rounded-2xl p-6">
        <h2 className="mb-4 text-lg font-semibold">{t('support.knowledgeBase.categories')}</h2>
        <div className="flex flex-wrap gap-2">
          {CATEGORIES.map(({ id, nameKey }) => (
            <Button
              key={id}
              onClick={() => setSelectedCategory(id)}
              className={cn(
                'rounded-lg px-3 py-1.5 text-xs font-medium transition-colors duration-150 h-auto',
                selectedCategory === id
                  ? 'bg-brand-soft text-white'
                  : 'bg-white/5 text-foreground/70 hover:bg-white/10'
              )}
            >
              {t(nameKey)}
            </Button>
          ))}
        </div>
      </div>

      <div className="space-y-3">
        {filteredArticles.length === 0 ? (
          <EmptyState
            icon={Search}
            title={t('support.logs.noArticlesFound')}
            className="glass-card rounded-2xl"
          />
        ) : (
          filteredArticles.map((article) => <KBArticleCard key={article.id} article={article} />)
        )}
      </div>
    </div>
  );
}
