import { useTranslation } from '@/lib/i18n';

/** Small inline pill marking a wizard control as not-yet-available. */
export function ComingSoonBadge() {
  const { t } = useTranslation();
  return (
    <span className="rounded-full border border-amber-400/20 bg-amber-400/10 px-1.5 py-0.5 text-[9px] font-semibold uppercase tracking-wide text-amber-300/90">
      {t('autopilot.wizard.comingSoon')}
    </span>
  );
}
