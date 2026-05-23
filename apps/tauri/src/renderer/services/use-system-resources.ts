import { useMemo } from 'react';
import { useSystemMetrics } from './use-system';
import { formatBytes, getDeviceTier, type ModelRec, MODEL_RECS } from '@ajh/shared';

export interface SystemResources {
  totalRamGb: number;
  usedRamGb: number;
  freeRamGb: number;
  cpuCount?: number;
  hasGpu: boolean;
  totalVramGb: number;
  usedVramGb: number;
  freeVramGb: number;
  deviceTier: { label: string; color: string };
}

export interface ModelResourceUsage {
  selectedModelRec: ModelRec | undefined;
  estimatedRamUsage: number;
  estimatedVramUsage: number;
  ramAfterModel: number;
  vramAfterModel: number;
  tooHeavy: boolean;
  mightLagRam: boolean;
  mightLagVram: boolean;
}

/**
 * Hook to calculate system resources and model usage
 */
export function useSystemResources(selectedModel?: string) {
  const { data: metricsRaw } = useSystemMetrics();
  const metrics = metricsRaw as
    | {
        totalMemoryMb?: number;
        memoryMb?: number;
        cpuCount?: number;
        gpus?: Array<{
          name: string;
          vramTotal: number;
          vramUsed: number;
          vramFree: number;
        }>;
      }
    | undefined;

  const resources = useMemo((): SystemResources => {
    const totalRamGb = Math.round((metrics?.totalMemoryMb ?? 8192) / 1024);
    const usedRamGb = Math.round((metrics?.memoryMb ?? 4096) / 1024);
    const freeRamGb = totalRamGb - usedRamGb;
    const cpuCount = metrics?.cpuCount;
    const gpus = metrics?.gpus ?? [];
    const hasGpu = gpus.length > 0;
    const totalVramGb = hasGpu ? Math.round((gpus[0]?.vramTotal ?? 0) / 1024 / 1024) : 0;
    const usedVramGb = hasGpu ? Math.round((gpus[0]?.vramUsed ?? 0) / 1024 / 1024) : 0;
    const freeVramGb = totalVramGb - usedVramGb;
    const deviceTier = getDeviceTier(totalRamGb, cpuCount);

    return {
      totalRamGb,
      usedRamGb,
      freeRamGb,
      cpuCount,
      hasGpu,
      totalVramGb,
      usedVramGb,
      freeVramGb,
      deviceTier,
    };
  }, [metrics]);

  const modelUsage = useMemo((): ModelResourceUsage => {
    if (!selectedModel) {
      return {
        selectedModelRec: undefined,
        estimatedRamUsage: 0,
        estimatedVramUsage: 0,
        ramAfterModel: resources.freeRamGb,
        vramAfterModel: resources.freeVramGb,
        tooHeavy: false,
        mightLagRam: false,
        mightLagVram: false,
      };
    }

    const selectedModelRec = MODEL_RECS.find((m: ModelRec) => m.name === selectedModel);
    const estimatedRamUsage = selectedModelRec?.estimatedRamGb ?? 0;
    const estimatedVramUsage = resources.hasGpu ? (selectedModelRec?.estimatedVramGb ?? 0) : 0;
    const ramAfterModel = resources.freeRamGb - estimatedRamUsage;
    const vramAfterModel = resources.hasGpu ? resources.freeVramGb - estimatedVramUsage : 0;
    const tooHeavy = (selectedModelRec?.minRamGb ?? 0) > resources.totalRamGb + 2;
    const mightLagRam = !tooHeavy && (selectedModelRec?.estimatedRamGb ?? 0) > resources.freeRamGb;
    const mightLagVram =
      resources.hasGpu &&
      (selectedModelRec?.estimatedVramGb ?? 0) > 0 &&
      (selectedModelRec?.estimatedVramGb ?? 0) > resources.freeVramGb;

    return {
      selectedModelRec,
      estimatedRamUsage,
      estimatedVramUsage,
      ramAfterModel,
      vramAfterModel,
      tooHeavy,
      mightLagRam,
      mightLagVram,
    };
  }, [selectedModel, resources]);

  return { resources, modelUsage, formatBytes };
}
