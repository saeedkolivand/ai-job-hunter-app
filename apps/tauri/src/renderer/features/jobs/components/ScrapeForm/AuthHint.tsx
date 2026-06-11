import { useTranslation } from '@ajh/translations';
import { SetupHint } from '@ajh/ui';

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
