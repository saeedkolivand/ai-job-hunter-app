import { Settings } from 'lucide-react';
import { Link } from '@tanstack/react-router';

import { useTranslation } from '@ajh/translations';

import { ROUTES } from '@/constants/routes';

interface PrefilledBadgeProps {
  field: string;
}

export function PrefilledBadge({ field }: PrefilledBadgeProps) {
  const { t } = useTranslation();
  return (
    <Link
      to={ROUTES.SETTINGS}
      className="inline-flex items-center gap-1 rounded-full bg-brand/10 px-2 py-0.5 text-[9px] font-medium text-brand-soft/70 hover:text-brand-soft transition-colors"
      title="Edit in Settings → Jobs"
    >
      <Settings size={8} /> {t('autopilot.wizard.target.fromLocationSettings')} {field}
    </Link>
  );
}
