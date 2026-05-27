/**
 * OCR worker — runs tesseract.js off the main thread so the main process
 * stays responsive during image recognition.
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
    const { createWorker } = await import('tesseract.js');
    const lang = msg.payload.lang ?? 'eng';
    const worker = await createWorker(lang);
    try {
      const { data } = await worker.recognize(msg.payload.path);
      const result: Output = { text: data.text, confidence: data.confidence, lang };
      port.postMessage({ id: msg.id, ok: true, result });
    } finally {
      await worker.terminate();
    }
  } catch (err) {
    port.postMessage({
      id: msg.id,
      ok: false,
      error: err instanceof Error ? err.message : String(err),
    });
  }
});
