export type ModelKind = 'reasoning' | 'embedding';

export const DEFAULT_MODELS = {
  reasoning: ['qwen3', 'llama3', 'mistral'] as const,
  embedding: ['bge-m3', 'multilingual-e5', 'nomic-embed-text'] as const,
} as const;

/** Embedding dimensions are discovered at runtime — NEVER hardcoded. */
export interface ModelInfo {
  name: string;
  kind: ModelKind;
  dimensions?: number;
  contextWindow?: number;
  loadedAt?: number;
  lastUsedAt?: number;
}
