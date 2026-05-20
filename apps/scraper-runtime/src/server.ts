/**
 * Scraper sidecar HTTP server.
 *
 * Routes:
 *   POST /command  — accept ScraperCommand, stream ScraperEvents via SSE.
 *   GET  /health   — return ScraperRuntimeHealth as JSON.
 *
 * Scraping is stubbed here — this file establishes the protocol proof.
 * Real scraper implementations (importing from @ajh/data) slot in by
 * implementing handleScrapeBoard / handleScrapeUrl below.
 */
import type { IncomingMessage, ServerResponse } from 'node:http';

import type {
  ScraperCatalogEntry,
  ScraperCommand,
  ScraperEvent,
  ScraperRuntimeHealth,
} from './protocol.js';

let _port = 0;
export function setServerPort(port: number): void {
  _port = port;
}

// ── SSE helpers ───────────────────────────────────────────────────────────────

function sendEvent(res: ServerResponse, event: ScraperEvent): void {
  res.write(`event: ${event.kind}\ndata: ${JSON.stringify(event)}\n\n`);
}

// ── Scraper catalog ───────────────────────────────────────────────────────────
// Extend this list as real HTTP scraper adapters are ported from @ajh/data.

const CATALOG: ScraperCatalogEntry[] = [
  { id: 'linkedin', displayName: 'LinkedIn', mode: 'http' },
  { id: 'indeed', displayName: 'Indeed', mode: 'http' },
  { id: 'stepstone', displayName: 'StepStone', mode: 'http' },
];

// ── Command handlers ──────────────────────────────────────────────────────────

async function handleCommand(cmd: ScraperCommand, res: ServerResponse): Promise<void> {
  switch (cmd.kind) {
    case 'scrape.board': {
      const { jobId, payload } = cmd;
      // Protocol proof: emit progress ticks then a done event.
      // Replace with real scraper call once @ajh/data HTTP scrapers are wired.
      for (let i = 1; i <= 3; i++) {
        sendEvent(res, { kind: 'progress', jobId, p: i / 3 });
        await sleep(100);
      }
      sendEvent(res, {
        kind: 'done',
        jobId,
        result: {
          board: payload.board,
          count: 0,
          note: 'sidecar stub — real scraper not yet wired',
        },
      });
      break;
    }

    case 'scrape.url': {
      const { jobId, payload } = cmd;
      // Stub — real scraper would fetch and parse the posting.
      sendEvent(res, {
        kind: 'done',
        jobId,
        result: { url: payload.url, note: 'sidecar stub — real scraper not yet wired' },
      });
      break;
    }

    case 'health': {
      const health: ScraperRuntimeHealth = {
        mode: 'http-sidecar',
        scrapers: CATALOG,
        ready: true,
        port: _port,
      };
      sendEvent(res, { kind: 'health.reply', health });
      break;
    }

    case 'catalog': {
      sendEvent(res, { kind: 'catalog.reply', scrapers: CATALOG });
      break;
    }

    default: {
      const exhaustive: never = cmd;
      const jobId = (exhaustive as { jobId?: string }).jobId ?? 'unknown';
      sendEvent(res, { kind: 'error', jobId, message: `Unknown command kind` });
    }
  }
}

// ── Body reader ───────────────────────────────────────────────────────────────

function readBody(req: IncomingMessage): Promise<string> {
  return new Promise((resolve, reject) => {
    const chunks: Buffer[] = [];
    req.on('data', (c: Buffer) => chunks.push(c));
    req.on('end', () => resolve(Buffer.concat(chunks).toString('utf8')));
    req.on('error', reject);
  });
}

function sleep(ms: number): Promise<void> {
  return new Promise((r) => setTimeout(r, ms));
}

// ── Request router ────────────────────────────────────────────────────────────

export function createRequestHandler() {
  return async (req: IncomingMessage, res: ServerResponse): Promise<void> => {
    // Only allow localhost connections.
    const remoteAddr = req.socket.remoteAddress ?? '';
    if (remoteAddr !== '127.0.0.1' && remoteAddr !== '::1' && remoteAddr !== '::ffff:127.0.0.1') {
      res.writeHead(403);
      res.end();
      return;
    }

    const pathname = new URL(req.url ?? '/', 'http://localhost').pathname;

    if (pathname === '/health' && req.method === 'GET') {
      const health: ScraperRuntimeHealth = {
        mode: 'http-sidecar',
        scrapers: CATALOG,
        ready: true,
        port: _port,
      };
      res.writeHead(200, { 'Content-Type': 'application/json' });
      res.end(JSON.stringify(health));
      return;
    }

    if (pathname === '/command' && req.method === 'POST') {
      let cmd: ScraperCommand;
      try {
        const body = await readBody(req);
        cmd = JSON.parse(body) as ScraperCommand;
      } catch {
        res.writeHead(400, { 'Content-Type': 'application/json' });
        res.end(JSON.stringify({ error: 'invalid JSON body' }));
        return;
      }

      res.writeHead(200, {
        'Content-Type': 'text/event-stream',
        'Cache-Control': 'no-cache',
        Connection: 'keep-alive',
        'X-Accel-Buffering': 'no',
      });

      try {
        await handleCommand(cmd, res);
      } catch (err) {
        const jobId = (cmd as { jobId?: string }).jobId ?? 'unknown';
        sendEvent(res, { kind: 'error', jobId, message: String(err) });
      }

      res.end();
      return;
    }

    res.writeHead(404);
    res.end();
  };
}
