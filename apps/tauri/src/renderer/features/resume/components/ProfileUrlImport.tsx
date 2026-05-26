import { useTranslation } from '@/lib/i18n';

interface Props {
  onImported?: (name: string) => void;
}

export function ProfileUrlImport(_props: Props) {
  const { t } = useTranslation();
  return (
    <p className="text-xs text-foreground/40 italic">
      {t('resume.profileImport.comingSoon', 'LinkedIn import is coming soon.')}
    </p>
  );
}
