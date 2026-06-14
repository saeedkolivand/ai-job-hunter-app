import { useEffect } from 'react';
import { useNavigate } from '@tanstack/react-router';

import { useSessionStore } from '@/store/session-store';

import { ApplyPage } from '../ApplyPage';

/**
 * Route component for `/autopilot/apply`. The apply target lives in the session
 * store (set by `AutopilotPage` when the user clicks "Apply" on a found job),
 * so this route is only meaningful in-session. On a cold URL (no `apply`
 * target — e.g. refresh or deep-link) it redirects back to `/autopilot` once,
 * rendering nothing while the redirect lands.
 */
export function ApplyPageRoute() {
  const apply = useSessionStore((s) => s.autopilot.apply);
  const setAutopilot = useSessionStore((s) => s.setAutopilot);
  const navigate = useNavigate();

  // Cold URL with no apply target → bounce back to the autopilot list. `replace`
  // keeps the back button sane (the apply route never enters history).
  useEffect(() => {
    if (!apply) void navigate({ to: '/autopilot', replace: true });
  }, [apply, navigate]);

  if (!apply) return null;

  const handleBack = () => {
    setAutopilot({ apply: null, applyWizardStep: 0, applyWizardForm: null });
    void navigate({ to: '/autopilot' });
  };

  return (
    <ApplyPage
      job={apply.job}
      resumeText={apply.resumeText}
      board={apply.board}
      onBack={handleBack}
    />
  );
}
