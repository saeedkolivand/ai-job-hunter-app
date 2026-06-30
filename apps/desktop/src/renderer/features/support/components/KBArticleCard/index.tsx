interface KBArticleCardProps {
  article: {
    id: number;
    title: string;
    category: string;
    summary: string;
  };
}

export function KBArticleCard({ article }: KBArticleCardProps) {
  return (
    <div className="surface-card rounded-xl p-4 hover:bg-muted transition-colors cursor-pointer">
      <div className="flex items-start gap-3">
        <div className="flex-1">
          <div className="text-sm font-medium text-foreground/90 mb-1">{article.title}</div>
          <div className="text-xs text-foreground/55">{article.summary}</div>
          <div className="mt-2">
            <span className="text-[10px] uppercase tracking-wider text-brand-soft">
              {article.category}
            </span>
          </div>
        </div>
      </div>
    </div>
  );
}
