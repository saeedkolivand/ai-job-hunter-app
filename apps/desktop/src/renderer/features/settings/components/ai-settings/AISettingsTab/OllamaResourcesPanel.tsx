import { useSystemResources } from '@/services';

interface Props {
  selectedModel?: string;
}

/** RAM/VRAM summary + lag warnings for the selected Ollama model. */
export function OllamaResourcesPanel({ selectedModel }: Props) {
  const { resources, modelUsage } = useSystemResources(selectedModel);
  const { totalRamGb, freeRamGb, deviceTier, hasGpu, freeVramGb } = resources;
  const { mightLagRam, mightLagVram } = modelUsage;

  return (
    <div className="space-y-2">
      {/* System resources display */}
      <div className="rounded-lg border border-foreground/10 bg-foreground/[0.03] px-3 py-2">
        <div className="flex items-center justify-between text-xs">
          <span className="text-foreground/40">
            RAM: {totalRamGb} GB ({freeRamGb} GB free)
          </span>
          <span className={`font-medium ${deviceTier.color}`}>{deviceTier.label}</span>
        </div>
        {hasGpu && (
          <div className="mt-1 text-xs text-foreground/40">VRAM: {freeVramGb} GB free</div>
        )}
        {mightLagRam && (
          <div className="mt-1 text-xs text-amber-400/80">
            ⚠️ Selected model may lag due to limited RAM
          </div>
        )}
        {mightLagVram && (
          <div className="mt-1 text-xs text-orange-400/80">
            ⚠️ Selected model may lag due to limited VRAM
          </div>
        )}
      </div>
    </div>
  );
}
