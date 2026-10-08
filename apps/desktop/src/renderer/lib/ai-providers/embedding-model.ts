/**
 * True for an Ollama model that can only embed (no chat endpoint). Mirrors the
 * backend's `is_embedding_only_model`
 * (`src-tauri/src/commands/ai_provider/ollama/models.rs`): every embedding model
 * Ollama publishes carries `embed` in its name. Keep the two in step.
 */
export function isEmbeddingOnlyModel(name: string): boolean {
  return name.toLowerCase().includes('embed');
}

/**
 * The models fit for a CHAT picker: drops embedding-only ones, except `keep`
 * (the user's saved model) so an existing selection stays visible rather than
 * reading as lost.
 */
export function chatModels<T extends { name: string }>(models: T[], keep?: string): T[] {
  return models.filter((m) => m.name === keep || !isEmbeddingOnlyModel(m.name));
}
