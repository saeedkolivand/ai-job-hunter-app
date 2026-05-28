import type { AiGenerateRequest } from '../../schemas/index.js';
import type { AiStreamChunk } from '../../types/index.js';

export interface AiContract {
  generate(req: AiGenerateRequest): Promise<{ jobId: string }>;

  /**
   * Stream a generation through the backend orchestration pipeline. Same wire
   * shape as `generate`, but the work runs as a composable `Pipeline` (so feature
   * generators share one lifecycle). Used by resume/cover-letter generation.
   */
  generatePipeline(req: AiGenerateRequest): Promise<{ jobId: string }>;

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

  /**
   * Test whether a stored provider API key is valid by making a lightweight API call.
   * `baseUrl` is forwarded for OpenAI-compatible servers (LM Studio, vLLM, etc.).
   */
  testProviderKey(req: {
    provider: string;
    baseUrl?: string;
  }): Promise<{ success: boolean; error?: string }>;

  /**
   * Fetch available models from a cloud provider using its stored API key.
   * `baseUrl` is forwarded for OpenAI-compatible servers.
   */
  listProviderModels(req: { provider: string; baseUrl?: string }): Promise<Array<{ name: string }>>;

  /** Active embedding space, per-space vector counts, and document index coverage. */
  embeddingStatus(): Promise<EmbeddingStatus>;

  /**
   * Set the active embedding provider/model. An empty model resolves to the
   * provider's default. Changing it changes the embedding space — call
   * `reembedAll` afterwards to rebuild the index.
   */
  setEmbeddingConfig(req: {
    provider: string;
    model?: string;
    baseUrl?: string;
  }): Promise<{ success: boolean; error?: string; config?: EmbeddingConfig }>;

  /** Re-embed every document with the active embedding config. Returns a job id. */
  reembedAll(): Promise<{ jobId: string }>;
}

export interface EmbeddingConfig {
  provider: string;
  model: string;
  baseUrl?: string | null;
}

export interface EmbeddingSpaceInfo {
  provider: string;
  model: string;
  dim: number;
  count: number;
  active: boolean;
}

export interface EmbeddingStatus {
  active: EmbeddingConfig;
  spaces: EmbeddingSpaceInfo[];
  documents: { total: number; indexedInActiveSpace: number; stale: number };
}

export const AI_CHANNELS = {
  generate: 'ai:generate',
  stream: 'ai:stream',
  listModels: 'ai:listModels',
  pullModel: 'ai:pullModel',
  unloadModel: 'ai:unloadModel',
  embed: 'ai:embed',
} as const;
