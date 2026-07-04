import type { AiGenerateRequest, ModelInspectResult } from '../../schemas/index.js';
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

  /**
   * Inspect a local (Ollama) model's real context window + size via `/api/show`,
   * to suggest safe generation limits. Returns `null` for non-local providers or
   * an unreachable Ollama server.
   */
  inspectModel(req: { model: string }): Promise<ModelInspectResult | null>;

  /**
   * Research the company named in a job ad and return a short factual brief —
   * used by the cover-letter "fit" paragraph and company-specific application
   * answers. Reuses the shared enricher: the active provider's own web search
   * (native tool, or the Ollama Web Search API for Ollama), cached. Degrades
   * gracefully — an empty brief, never an error, when the provider can't search
   * or the search fails. The brief is reference context only; the prompt treats
   * it as untrusted.
   */
  researchCompany(req: {
    jobAd: string;
    /** Accurate AI-extracted company name; preferred over heuristic job-ad extraction. */
    company?: string;
    provider?: string;
    model?: string;
    baseUrl?: string;
  }): Promise<{ company: string; brief: string }>;

  /**
   * Web-grounded market salary-range lookup for the salary application
   * question. Reuses the active provider's own web search (same channel as
   * `researchCompany`), parsed and strictly validated server-side, cached.
   * Degrades gracefully — `null`, never an error, when the provider can't
   * search, the search yields nothing reliable, or times out. Only validated
   * integers + a sanitized currency code are ever returned; no raw web text
   * crosses this boundary.
   */
  lookupSalary(req: {
    role: string;
    company?: string;
    location?: string;
    provider?: string;
    model?: string;
    baseUrl?: string;
  }): Promise<SalaryRange | null>;

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

/** A validated web-researched market salary range (mirrors the Rust
 *  `salary_research::SalaryRange` — min/max/currency only, already validated
 *  server-side before it crosses the IPC boundary). */
export interface SalaryRange {
  min: number;
  max: number;
  currency: string;
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
  listModels: 'ai:listModels',
  pullModel: 'ai:pullModel',
  unloadModel: 'ai:unloadModel',
  embed: 'ai:embed',
} as const;
