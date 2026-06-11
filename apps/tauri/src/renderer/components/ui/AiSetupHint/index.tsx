import { useRouter } from '@tanstack/react-router';

import { useTranslation } from '@ajh/translations';
import { SetupHint } from '@ajh/ui';

import { ROUTES } from '@/constants/routes/routes';
import { useSessionStore } from '@/store/session-store';

/** Maps a {@link useCanUseAI} block reason to its hint copy. */
const MESSAGE_KEY: Record<string, string> = {
  addApiKey: 'aiSetup.addApiKey',
  selectModel: 'aiSetup.selectModel',
  installCli: 'aiSetup.installCli',
};

interface Props {
  /** Whether AI is currently blocked (typically `!canUseAI`). */
  show: boolean;
  /** The block reason from `useCanUseAI` (`addApiKey` | `selectModel` | `installCli`). */
  reason?: string;
}

/**
 * One-click setup nudge shown when an AI provider isn't ready, so the user can
 * jump straight to the AI settings section instead of hunting for it. The fix
 * for every reason lives in the same section, so the action is uniform.
 */
export function AiSetupHint({ show, reason }: Props) {
  const { t } = useTranslation();
  const router = useRouter();
  const setSettings = useSessionStore((s) => s.setSettings);

  const openAiSettings = () => {
    setSettings({ activeSection: 'ai' });
    void router.navigate({ to: ROUTES.SETTINGS });
  };

  return (
    <SetupHint
      show={show}
      message={t((reason && MESSAGE_KEY[reason]) || 'aiSetup.addApiKey')}
      actionLabel={t('aiSetup.action')}
      onAction={openAiSettings}
    />
  );
}
