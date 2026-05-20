/**
 * Embeddings worker — batches calls into Ollama from a worker thread, so
 * the main process stays responsive during large indexing jobs.
 *
 * Stub forwards to a no-op until the AI runtime is wired through an IPC-style
 * channel (workers don't share JS state with the main thread).
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
    // TODO: dynamic-import @ajh/ai inside worker once compiled output is wired.
    const result: Output = { vectors: msg.payload.texts.map(() => []), dim: 0 };
    port.postMessage({ id: msg.id, ok: true, result });
  } catch (err) {
    port.postMessage({
      id: msg.id,
      ok: false,
      error: err instanceof Error ? err.message : String(err),
    });
  }
});
