import type { AiGenerateRequest } from '../../schemas/index.js';
import type { AiStreamChunk } from '../../types/index.js';

export interface AiContract {
  generate(req: AiGenerateRequest): Promise<{ jobId: string }>;

  onStream(handler: (chunk: AiStreamChunk) => void): () => void;

  listModels(): Promise<Array<{ name: string }>>;

  pullModel(model: string): Promise<{ jobId: string }>;

  unloadModel(model: string): Promise<void>;

  /** Synchronous embedding — returns the vector. Falls back gracefully if Ollama is offline. */
  embed(req: { text: string; model?: string }): Promise<{ vector: number[]; dim: number } | null>;

  /** Store an API key for a cloud AI provider in the OS keychain. */
  setProviderKey(req: { provider: string; apiKey: string }): Promise<{ success: boolean }>;

  /** Remove a stored provider API key from the OS keychain. */
  removeProviderKey(req: { provider: string }): Promise<{ success: boolean }>;

  /** Check whether a provider API key is stored (does not return the key). */
  hasProviderKey(req: { provider: string }): Promise<{ has: boolean }>;

  /** Fetch available models from a cloud provider using its stored API key. */
  listProviderModels(req: { provider: string }): Promise<Array<{ name: string }>>;
}

export const AI_CHANNELS = {
  generate: 'ai:generate',
  stream: 'ai:stream',
  listModels: 'ai:listModels',
  pullModel: 'ai:pullModel',
  unloadModel: 'ai:unloadModel',
  embed: 'ai:embed',
} as const;
