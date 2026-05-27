/**
 * Embeddings worker — batches Ollama embed calls off the main thread so
 * the main process stays responsive during large indexing jobs.
 */
import { parentPort } from 'node:worker_threads';

if (!parentPort) throw new Error('embeddings.worker must run as a worker thread');
const port = parentPort;

interface Input {
  model: string;
  texts: string[];
  host?: string;
}
interface Output {
  vectors: number[][];
  dim: number;
}

port.on('message', async (msg: { id: string; payload: Input }) => {
  try {
    const { OllamaClient, embedBatch } = await import('@ajh/ai');
    const client = new OllamaClient({ host: msg.payload.host });
    const vectors = await embedBatch(client, msg.payload.model, msg.payload.texts);
    const result: Output = { vectors, dim: vectors[0]?.length ?? 0 };
    port.postMessage({ id: msg.id, ok: true, result });
  } catch (err) {
    port.postMessage({
      id: msg.id,
      ok: false,
      error: err instanceof Error ? err.message : String(err),
    });
  }
});
