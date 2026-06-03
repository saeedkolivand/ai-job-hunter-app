import { SetupHint } from '@ajh/ui';

import { useTranslation } from '@/lib/i18n';

interface Props {
  show: boolean;
  connectPending: boolean;
  onConnect: () => void;
}

export function AuthHint({ show, connectPending, onConnect }: Props) {
  const { t } = useTranslation();

  return (
    <SetupHint
      show={show}
      message={t('jobs.authHint')}
      actionLabel={t('jobs.authHintLink')}
      onAction={onConnect}
      pending={connectPending}
    />
  );
}
