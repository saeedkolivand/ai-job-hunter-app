/**
 * Embeddings — model-aware. Dimensions are discovered, never hardcoded.
 */
import type { OllamaClient } from '../client/ollama.js';

export async function embed(client: OllamaClient, model: string, text: string): Promise<number[]> {
  const vectors = await client.embed(model, text);
  const v = vectors[0];
  if (!v) throw new Error(`Embedding failed for model ${model}`);
  return v;
}

export async function embedBatch(
  client: OllamaClient,
  model: string,
  texts: string[],
  opts: { batchSize?: number; onProgress?: (done: number, total: number) => void } = {}
): Promise<number[][]> {
  const batchSize = opts.batchSize ?? 16;
  const out: number[][] = [];
  for (let i = 0; i < texts.length; i += batchSize) {
    const slice = texts.slice(i, i + batchSize);
    const vectors = await client.embed(model, slice);
    out.push(...vectors);
    opts.onProgress?.(Math.min(i + batchSize, texts.length), texts.length);
  }
  return out;
}
