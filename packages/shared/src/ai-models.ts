/**
 * AI Model recommendations with resource requirements
 */
export interface ModelRec {
  name: string;
  label: string;
  description: string;
  sizeGb: number;
  minRamGb: number;
  estimatedRamGb: number; // Actual RAM usage when loaded
  estimatedVramGb?: number; // VRAM usage if GPU available
}

export const MODEL_RECS: ModelRec[] = [
  {
    name: 'llama3.2:1b',
    label: 'Llama 3.2 (1B)',
    description: 'Ultra-lightweight — works on almost any device',
    sizeGb: 1.3,
    minRamGb: 4,
    estimatedRamGb: 2,
    estimatedVramGb: 1.5,
  },
  {
    name: 'llama3.2',
    label: 'Llama 3.2 (3B)',
    description: 'Great balance of speed and quality for everyday tasks',
    sizeGb: 2.0,
    minRamGb: 6,
    estimatedRamGb: 4,
    estimatedVramGb: 3,
  },
  {
    name: 'mistral',
    label: 'Mistral 7B',
    description: 'Strong reasoning, ideal for resume analysis',
    sizeGb: 4.1,
    minRamGb: 10,
    estimatedRamGb: 8,
    estimatedVramGb: 6,
  },
  {
    name: 'llama3.1:8b',
    label: 'Llama 3.1 (8B)',
    description: 'Best quality for powerful machines',
    sizeGb: 4.7,
    minRamGb: 12,
    estimatedRamGb: 10,
    estimatedVramGb: 8,
  },
];

/**
 * Get recommended model based on available RAM
 */
export function getRecommended(totalRamGb: number): ModelRec {
  if (totalRamGb >= 12)
    return (
      MODEL_RECS[3] ??
      MODEL_RECS[0] ??
      MODEL_RECS[1] ??
      MODEL_RECS[2] ?? {
        name: 'llama3.2',
        label: 'Llama 3.2',
        description: '',
        sizeGb: 2,
        minRamGb: 6,
        estimatedRamGb: 4,
        estimatedVramGb: 3,
      }
    );
  if (totalRamGb >= 10)
    return (
      MODEL_RECS[2] ??
      MODEL_RECS[1] ??
      MODEL_RECS[0] ?? {
        name: 'mistral',
        label: 'Mistral 7B',
        description: '',
        sizeGb: 4.1,
        minRamGb: 10,
        estimatedRamGb: 8,
        estimatedVramGb: 6,
      }
    );
  if (totalRamGb >= 6)
    return (
      MODEL_RECS[1] ??
      MODEL_RECS[0] ?? {
        name: 'llama3.2',
        label: 'Llama 3.2',
        description: '',
        sizeGb: 2,
        minRamGb: 6,
        estimatedRamGb: 4,
        estimatedVramGb: 3,
      }
    );
  return (
    MODEL_RECS[0] ?? {
      name: 'llama3.2:1b',
      label: 'Llama 3.2 (1B)',
      description: '',
      sizeGb: 1.3,
      minRamGb: 4,
      estimatedRamGb: 2,
      estimatedVramGb: 1.5,
    }
  );
}
