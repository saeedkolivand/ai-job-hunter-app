import { Download, ExternalLink, Loader2 } from 'lucide-react';

import { useTranslation } from '@ajh/translations';
import { Button, Dropdown } from '@ajh/ui';

import type { Model } from '@/types';

const QUICK_MODELS = ['llama3.2', 'mistral', 'llama3.1:8b', 'llama3.2:1b'];

interface Props {
  connected: boolean;
  models: Model[];
  providerModel: string;
  loading: boolean;
  pulling: string | null;
  onPull: (model: string) => void;
  onSelect: (model: string) => void;
  onSetActive: () => void;
  isActive: boolean;
  onDownloadOllama: () => void;
  onRecheck: () => void;
  children?: React.ReactNode;
}

export function OllamaConfig({
  connected,
  models,
  providerModel,
  loading,
  pulling,
  onPull,
  onSelect,
  onSetActive,
  isActive,
  onDownloadOllama,
  onRecheck,
  children,
}: Props) {
  const { t } = useTranslation();

  return (
    <>
      {!connected && (
        <div className="flex gap-2">
          <Button variant="ghost" className="text-foreground/50" onClick={onDownloadOllama}>
            <ExternalLink size={11} /> Download Ollama
          </Button>
          <Button variant="ghost" className="text-foreground/40" onClick={onRecheck}>
            <Loader2 size={11} /> Recheck
          </Button>
        </div>
      )}
      <div className="flex items-center justify-between">
        <span className="text-xs font-semibold uppercase tracking-[0.16em] text-foreground/55">
          {t('settings.aiModel.title')}
        </span>
        <Button
          onClick={onRecheck}
          disabled={loading}
          className="flex items-center gap-1.5 rounded-lg bg-foreground/[0.06] px-2 py-1 text-xs text-foreground/60 hover:text-foreground h-auto border-transparent"
        >
          <Loader2 size={11} className={loading ? 'animate-spin' : ''} />
          {t('settings.aiModel.refresh')}
        </Button>
      </div>
      {models.length === 0 ? (
        <div className="space-y-2">
          <p className="text-sm text-foreground/50">{t('settings.aiModel.noModels')}</p>
          {connected &&
            QUICK_MODELS.map((qm) => (
              <Button
                key={qm}
                variant="unstyled"
                onClick={() => void onPull(qm)}
                disabled={pulling !== null}
                className="flex w-full items-center justify-between rounded-lg border border-foreground/10 bg-foreground/[0.03] px-3 py-2 text-left text-sm hover:bg-foreground/[0.05] disabled:opacity-50"
              >
                <span className="text-foreground/70">{qm}</span>
                {pulling === qm ? (
                  <Loader2 size={13} className="animate-spin text-brand-soft" />
                ) : (
                  <Download size={13} className="text-foreground/30" />
                )}
              </Button>
            ))}
        </div>
      ) : (
        <Dropdown
          options={models.map((m) => ({ value: m.name, label: m.name }))}
          value={providerModel}
          onChange={onSelect}
          placeholder="Select a model…"
        />
      )}
      {connected && (
        <div className="space-y-2">
          {children}
          <Button
            variant="glass"
            onClick={onSetActive}
            disabled={isActive}
            className={isActive ? 'opacity-40' : 'ring-1 ring-brand/20'}
          >
            {isActive ? 'Currently active' : 'Set as active'}
          </Button>
        </div>
      )}
    </>
  );
}
