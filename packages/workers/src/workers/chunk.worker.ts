/**
 * Chunking worker — splits long text into overlapping semantic chunks.
 * Keeps the main thread free during large document imports.
 */
import { parentPort } from 'node:worker_threads';

if (!parentPort) throw new Error('chunk.worker must run as a worker thread');
const port = parentPort;

interface Input {
  text: string;
  chunkSize?: number;
  overlap?: number;
}
interface Output {
  chunks: string[];
}

function chunk(text: string, size: number, overlap: number): string[] {
  if (text.length <= size) return [text];
  const parts: string[] = [];
  let i = 0;
  while (i < text.length) {
    const end = Math.min(text.length, i + size);
    // try to break on paragraph or sentence boundary
    let cut = end;
    if (end < text.length) {
      const slice = text.slice(i, end);
      const lastPara = slice.lastIndexOf('\n\n');
      const lastSent = Math.max(
        slice.lastIndexOf('. '),
        slice.lastIndexOf('! '),
        slice.lastIndexOf('? ')
      );
      cut = i + (lastPara > size * 0.5 ? lastPara : lastSent > size * 0.5 ? lastSent + 1 : end - i);
    }
    parts.push(text.slice(i, cut).trim());
    if (cut >= text.length) break;
    i = Math.max(cut - overlap, i + 1);
  }
  return parts.filter((p) => p.length > 0);
}

port.on('message', (msg: { id: string; payload: Input }) => {
  try {
    const { text, chunkSize = 1200, overlap = 150 } = msg.payload;
    const result: Output = { chunks: chunk(text, chunkSize, overlap) };
    port.postMessage({ id: msg.id, ok: true, result });
  } catch (err) {
    port.postMessage({
      id: msg.id,
      ok: false,
      error: err instanceof Error ? err.message : String(err),
    });
  }
});
