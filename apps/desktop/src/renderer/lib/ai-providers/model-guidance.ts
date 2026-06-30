import { getModelTier } from '@ajh/prompts/context-manager';

import type { ProviderKind } from './provider-meta';

/**
 * "Which model for what" guidance (#6) — derived entirely from the model name +
 * provider kind, so a NEW model gets sensible guidance with zero code changes
 * (registry/inference-driven, never a per-model-name table).
 *
 * Two signals:
 * - **tier** (`getModelTier`): cloud / CLI models and >14B locals → `large`;
 *   4–14B → `medium`; <4B / unknown local → `small`.
 * - **light**: a name that advertises a fast/economical variant (mini, flash,
 *   haiku, lite, nano, or a sub-4B local) — capable but trades depth for speed.
 *
 * The UI maps `task`/`kind` to i18n strings; this stays pure + testable.
 */
export interface ModelGuidance {
  tier: 'large' | 'medium' | 'small';
  light: boolean;
  /** i18n key suffix for the task-fit hint. */
  task: 'strong' | 'balanced' | 'basic' | 'light';
  /** i18n key suffix for the provider-kind note. */
  kind: 'cloud' | 'local' | 'cli';
}

const LIGHT_VARIANT_RE = /\b(mini|flash|lite|nano|small)\b|haiku/i;

function kindKey(kind: ProviderKind): ModelGuidance['kind'] {
  if (kind === 'cloud') return 'cloud';
  if (kind === 'cli-agent') return 'cli';
  return 'local';
}

export function getModelGuidance(modelName: string, kind: ProviderKind): ModelGuidance {
  const tier = getModelTier(modelName);
  const light = LIGHT_VARIANT_RE.test(modelName) || tier === 'small';

  // A light/fast variant is called out as such regardless of its nominal tier
  // (e.g. a hosted "mini" classifies as `large` but behaves like a fast variant).
  const task: ModelGuidance['task'] = light
    ? 'light'
    : tier === 'large'
      ? 'strong'
      : tier === 'medium'
        ? 'balanced'
        : 'basic';

  return { tier, light, task, kind: kindKey(kind) };
}
