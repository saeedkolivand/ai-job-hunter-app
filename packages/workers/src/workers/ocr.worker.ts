/**
 * OCR worker — runs tesseract.js off the main thread.
 * Stub: returns the input path with an empty `text` field until wired.
 */
import { parentPort } from 'node:worker_threads';

if (!parentPort) throw new Error('ocr.worker must run as a worker thread');
const port = parentPort;

interface Input {
  path: string;
  lang?: string;
}
interface Output {
  text: string;
  confidence: number;
  lang: string;
}

port.on('message', async (msg: { id: string; payload: Input }) => {
  try {
    // TODO: integrate tesseract.js. Loaded lazily to keep worker startup cheap.
    // const { createWorker } = await import('tesseract.js');
    // const worker = await createWorker(msg.payload.lang ?? 'eng');
    // const { data } = await worker.recognize(msg.payload.path);
    // await worker.terminate();
    const result: Output = { text: '', confidence: 0, lang: msg.payload.lang ?? 'eng' };
    port.postMessage({ id: msg.id, ok: true, result });
  } catch (err) {
    port.postMessage({
      id: msg.id,
      ok: false,
      error: err instanceof Error ? err.message : String(err),
    });
  }
});
