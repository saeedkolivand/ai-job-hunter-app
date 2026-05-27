import { useTranslation } from '@/lib/i18n';

interface LogCategoryCardProps {
  name: string;
  count: number;
  errors: number;
  warnings: number;
}

export function LogCategoryCard({ name, count, errors, warnings }: LogCategoryCardProps) {
  const { t } = useTranslation();

  return (
    <div className="glass-card rounded-xl p-4">
      <div className="text-sm font-medium text-foreground/90 mb-3">{name}</div>
      <div className="space-y-2">
        <div className="flex items-center justify-between text-xs">
          <span className="text-foreground/55">{t('support.logs.total')}</span>
          <span className="text-foreground/90">{count}</span>
        </div>
        <div className="flex items-center justify-between text-xs">
          <span className="text-foreground/55">{t('support.logs.errors')}</span>
          <span className={errors > 0 ? 'text-red-400' : 'text-emerald-400'}>{errors}</span>
        </div>
        <div className="flex items-center justify-between text-xs">
          <span className="text-foreground/55">{t('support.logs.warnings')}</span>
          <span className={warnings > 0 ? 'text-amber-400' : 'text-emerald-400'}>{warnings}</span>
        </div>
      </div>
    </div>
  );
}
