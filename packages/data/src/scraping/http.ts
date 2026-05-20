/**
 * Shared HTTP client for scrapers.
 *
 * - sane default headers (modern desktop UA)
 * - per-request abort signal honoured
 * - light retry on transient failures
 * - opt-in JSON / HTML helpers with size caps
 */
import { type Dispatcher, request } from 'undici';

const DEFAULT_UA =
  'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 ' +
  '(KHTML, like Gecko) Chrome/124.0.0.0 Safari/537.36';

const MAX_BYTES = 8 * 1024 * 1024; // 8 MB per response — generous, but capped

export interface FetchOptions {
  signal?: AbortSignal;
  headers?: Record<string, string>;
  method?: Dispatcher.HttpMethod;
  body?: string;
  retries?: number;
}

export interface FetchResult {
  statusCode: number;
  headers: Record<string, string | string[] | undefined>;
  text: string;
}

export async function fetchText(url: string, opts: FetchOptions = {}): Promise<FetchResult> {
  const retries = opts.retries ?? 1;
  let lastErr: unknown;
  for (let attempt = 0; attempt <= retries; attempt++) {
    try {
      const reqOpts: Parameters<typeof request>[1] = {
        method: opts.method ?? 'GET',
        headers: {
          'user-agent': DEFAULT_UA,
          accept:
            'text/html,application/xhtml+xml,application/xml,application/json;q=0.9,*/*;q=0.8',
          'accept-language': 'en-US,en;q=0.9,de;q=0.8',
          ...(opts.headers ?? {}),
        },
        ...(opts.signal ? { signal: opts.signal } : {}),
        ...(opts.body ? { body: opts.body } : {}),
      };
      const { statusCode, headers, body } = await request(url, reqOpts);
      const text = await readCapped(body);
      return { statusCode, headers, text };
    } catch (err) {
      lastErr = err;
      if (opts.signal?.aborted) throw err;
      if (attempt < retries) await delay(300 * (attempt + 1));
    }
  }
  throw lastErr instanceof Error ? lastErr : new Error('fetchText failed');
}

export async function fetchJson<T>(url: string, opts: FetchOptions = {}): Promise<T | null> {
  const res = await fetchText(url, {
    ...opts,
    headers: { accept: 'application/json', ...(opts.headers ?? {}) },
  });
  if (res.statusCode < 200 || res.statusCode >= 300) return null;
  try {
    return JSON.parse(res.text) as T;
  } catch {
    return null;
  }
}

async function readCapped(body: NodeJS.ReadableStream): Promise<string> {
  let total = 0;
  const chunks: Buffer[] = [];
  for await (const chunk of body) {
    const buf = typeof chunk === 'string' ? Buffer.from(chunk) : (chunk as Buffer);
    total += buf.length;
    if (total > MAX_BYTES) throw new Error('Response too large');
    chunks.push(buf);
  }
  return Buffer.concat(chunks).toString('utf8');
}

function delay(ms: number) {
  return new Promise((r) => setTimeout(r, ms));
}

export function stripHtml(html: string): string {
  return html
    .replace(/<script[\s\S]*?<\/script>/gi, ' ')
    .replace(/<style[\s\S]*?<\/style>/gi, ' ')
    .replace(/<[^>]+>/g, ' ')
    .replace(/&nbsp;/g, ' ')
    .replace(/&amp;/g, '&')
    .replace(/&lt;/g, '<')
    .replace(/&gt;/g, '>')
    .replace(/&quot;/g, '"')
    .replace(/&#39;/g, "'")
    .replace(/\s+/g, ' ')
    .trim();
}
