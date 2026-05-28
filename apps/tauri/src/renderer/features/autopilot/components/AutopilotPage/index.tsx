import { AlertCircle, Plus, X, Zap } from 'lucide-react';
import { AnimatePresence } from 'motion/react';

import type { Autopilot } from '@ajh/shared';
import { Button } from '@ajh/ui';

import { PageTransition } from '@/components/layout/PageTransition';
import { AutopilotCard } from '@/features/autopilot/components/AutopilotCard';
import { CreationWizard } from '@/features/autopilot/components/CreationWizard';
import { EmptyState } from '@/features/autopilot/components/EmptyState';
import { useAutopilotRun } from '@/features/autopilot/hooks/useAutopilotRun';
import { useTranslation } from '@/lib/i18n';
import { useAutopilots } from '@/services';
import { useSessionStore } from '@/store/session-store';

function AutopilotPage() {
  const { t } = useTranslation();
  const { data: autopilotList = [], isLoading: loading } = useAutopilots();
  const autopilots = autopilotList as Autopilot[];
  const { autopilot, setAutopilot, resetAutopilotWizard } = useSessionStore();
  const { creating } = autopilot;
  const setCreating = (v: boolean) => setAutopilot({ creating: v });
  const resetWizard = resetAutopilotWizard;

  const { runStates, stepLogs, error, setError, handleRun, handleTogglePause, handleDelete } =
    useAutopilotRun();

  const handleCreated = (_ap: Autopilot) => {
    resetWizard();
  };

  return (
    <PageTransition className="h-full overflow-hidden">
      <div className="flex h-full flex-col">
        {/* Header */}
        <div className="flex items-center justify-between px-8 py-5 shrink-0">
          <div className="flex items-center gap-3">
            <div className="flex h-8 w-8 items-center justify-center rounded-xl bg-brand/15">
              <Zap size={15} className="text-brand-soft" />
            </div>
            <div>
              <h1 className="text-base font-semibold text-foreground/90">{t('autopilot.title')}</h1>
              <p className="text-[11px] text-foreground/40">{t('autopilot.subtitle')}</p>
            </div>
          </div>
          <Button
            variant="glass"
            size="sm"
            onClick={() => setCreating(true)}
            className="transition-all duration-150 ease-out"
          >
            <Plus size={13} /> {t('autopilot.newAutopilot')}
          </Button>
        </div>

        {/* Content */}
        <div className="flex-1 overflow-y-auto px-8 py-6">
          {error && (
            <div className="mb-4 flex items-center gap-2 rounded-xl border border-red-400/20 bg-red-400/5 px-4 py-3 text-xs text-red-300/80">
              <AlertCircle size={12} /> {error}
              <Button
                onClick={() => setError(null)}
                className="ml-auto h-auto bg-transparent border-transparent p-0"
              >
                <X size={11} />
              </Button>
            </div>
          )}

          {loading ? (
            <div className="flex items-center justify-center h-40 text-foreground/30 text-sm">
              {t('autopilot.loading')}
            </div>
          ) : autopilots.length === 0 ? (
            <EmptyState onNew={() => setCreating(true)} />
          ) : (
            <div className="space-y-3">
              {autopilots.map((ap) => {
                const runState = runStates[ap._id] ?? 'idle';
                return (
                  <AutopilotCard
                    key={ap._id}
                    autopilot={ap}
                    runState={runState}
                    stepLogs={stepLogs[ap._id] ?? []}
                    onRun={() => void handleRun(ap._id)}
                    onTogglePause={() => void handleTogglePause(ap)}
                    onDelete={() => void handleDelete(ap._id)}
                  />
                );
              })}
            </div>
          )}
        </div>
      </div>

      {/* Creation wizard overlay */}
      <AnimatePresence>
        {creating && <CreationWizard onDone={handleCreated} onCancel={resetWizard} />}
      </AnimatePresence>
    </PageTransition>
  );
}

export { AutopilotPage };
