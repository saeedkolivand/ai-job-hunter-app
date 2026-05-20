/**
 * Scraper sidecar HTTP server.
 *
 * Routes:
 *   POST /command  — accept ScraperCommand, stream ScraperEvents via SSE.
 *   GET  /health   — return ScraperRuntimeHealth as JSON.
 *
 * The ScraperEngine (engine.ts) handles all real scraper calls.
 * FileCredentialStore (credentials.ts) provides credential access.
 */
import type { IncomingMessage, ServerResponse } from 'node:http';

import type { CollectionName } from '@ajh/data';

import type { ScraperEngine } from './engine.js';
import type { ScraperCommand, ScraperEvent, ScraperRuntimeHealth } from './protocol.js';

let _port = 0;
export function setServerPort(port: number): void {
  _port = port;
}

// ── SSE helpers ───────────────────────────────────────────────────────────────

function sendEvent(res: ServerResponse, event: ScraperEvent): void {
  res.write(`event: ${event.kind}\ndata: ${JSON.stringify(event)}\n\n`);
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

// ── Command handler ───────────────────────────────────────────────────────────

async function handleCommand(
  cmd: ScraperCommand,
  res: ServerResponse,
  engine: ScraperEngine
): Promise<void> {
  switch (cmd.kind) {
    case 'scrape.board': {
      const { jobId, payload } = cmd;
      try {
        const result = await engine.scrapeBoard(payload, jobId, (event) => sendEvent(res, event));
        sendEvent(res, { kind: 'done', jobId, result });
      } catch (err) {
        sendEvent(res, { kind: 'error', jobId, message: String(err) });
      }
      break;
    }

    case 'scrape.url': {
      const { jobId, payload } = cmd;
      try {
        const posting = await engine.scrapeUrl(payload.url, jobId, (event) =>
          sendEvent(res, event)
        );
        sendEvent(res, {
          kind: 'done',
          jobId,
          result: posting ?? { error: 'no scraper matched this URL' },
        });
      } catch (err) {
        sendEvent(res, { kind: 'error', jobId, message: String(err) });
      }
      break;
    }

    case 'cancel': {
      engine.cancel(cmd.jobId);
      sendEvent(res, { kind: 'done', jobId: cmd.jobId, result: { cancelled: true } });
      break;
    }

    case 'set.credentials': {
      const { boardId, username, password } = cmd;
      engine.setCredentials(boardId, username, password);
      sendEvent(res, {
        kind: 'done',
        jobId: 'credentials',
        result: { boardId, stored: true },
      });
      break;
    }

    case 'open.login': {
      const { boardId } = cmd;
      try {
        const result = await engine.openLogin(boardId, (event) => sendEvent(res, event));
        sendEvent(res, { kind: 'done', jobId: `login.${boardId}`, result });
      } catch (err) {
        sendEvent(res, {
          kind: 'error',
          jobId: `login.${boardId}`,
          message: String(err),
        });
      }
      break;
    }

    case 'board.status': {
      const { boardId } = cmd;
      const status = engine.getBoardStatus(boardId);
      sendEvent(res, { kind: 'done', jobId: `status.${boardId}`, result: status });
      break;
    }

    case 'board.disconnect': {
      const { boardId } = cmd;
      engine.disconnectBoard(boardId);
      sendEvent(res, { kind: 'done', jobId: `disconnect.${boardId}`, result: { success: true } });
      break;
    }

    case 'apply.job': {
      const { jobId, payload } = cmd;
      try {
        const result = await engine.applyJob(payload, jobId, (event) => sendEvent(res, event));
        sendEvent(res, { kind: 'done', jobId, result });
      } catch (err) {
        sendEvent(res, { kind: 'error', jobId, message: String(err) });
      }
      break;
    }

    case 'apply.catalog': {
      const catalog = engine.applierCatalog();
      sendEvent(res, { kind: 'done', jobId: 'apply.catalog', result: catalog });
      break;
    }

    case 'extract.text': {
      const { jobId, payload } = cmd;
      try {
        const result = await engine.extractText(payload.name, payload.bytesBase64);
        sendEvent(res, { kind: 'done', jobId, result });
      } catch (err) {
        sendEvent(res, { kind: 'error', jobId, message: String(err) });
      }
      break;
    }

    case 'document.import': {
      const { jobId, payload } = cmd;
      try {
        sendEvent(res, { kind: 'progress', jobId, p: 0.2 });
        // Extract text from bytes (reuse engine's extractText).
        const extracted = await engine.extractText(payload.name, payload.bytesBase64);
        sendEvent(res, { kind: 'progress', jobId, p: 0.6 });
        const doc = await engine.dataStore.importDocument(
          payload.name,
          extracted.text,
          payload.locale
        );
        sendEvent(res, { kind: 'progress', jobId, p: 1 });
        sendEvent(res, { kind: 'done', jobId, result: doc });
      } catch (err) {
        sendEvent(res, { kind: 'error', jobId, message: String(err) });
      }
      break;
    }

    case 'document.list': {
      const { jobId } = cmd;
      try {
        const docs = await engine.dataStore.listDocuments();
        sendEvent(res, { kind: 'done', jobId, result: docs });
      } catch (err) {
        sendEvent(res, { kind: 'error', jobId, message: String(err) });
      }
      break;
    }

    case 'document.remove': {
      const { jobId, payload } = cmd;
      try {
        await engine.dataStore.removeDocument(payload.id);
        sendEvent(res, { kind: 'done', jobId, result: { success: true } });
      } catch (err) {
        sendEvent(res, { kind: 'error', jobId, message: String(err) });
      }
      break;
    }

    case 'search.hybrid': {
      const { jobId, payload } = cmd;
      try {
        const results = await engine.dataStore.hybridSearch(
          payload.query,
          payload.collection as CollectionName,
          payload.topK
        );
        sendEvent(res, { kind: 'done', jobId, result: { items: results, total: results.length } });
      } catch (err) {
        sendEvent(res, { kind: 'error', jobId, message: String(err) });
      }
      break;
    }

    case 'match.resume': {
      const { jobId, payload } = cmd;
      try {
        const scores = await engine.dataStore.matchResume(payload.resumeId, payload.jobText);
        sendEvent(res, { kind: 'done', jobId, result: scores });
      } catch (err) {
        sendEvent(res, { kind: 'error', jobId, message: String(err) });
      }
      break;
    }

    case 'health': {
      const health: ScraperRuntimeHealth = { ...engine.health(), port: _port };
      sendEvent(res, { kind: 'health.reply', health });
      break;
    }

    case 'catalog': {
      sendEvent(res, { kind: 'catalog.reply', scrapers: engine.catalog() });
      break;
    }

    default: {
      const exhaustive: never = cmd;
      const jobId = (exhaustive as { jobId?: string }).jobId ?? 'unknown';
      sendEvent(res, { kind: 'error', jobId, message: 'unknown command kind' });
    }
  }
}

// ── Request router ────────────────────────────────────────────────────────────

export function createRequestHandler(engine: ScraperEngine) {
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
      const health: ScraperRuntimeHealth = { ...engine.health(), port: _port };
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

      await handleCommand(cmd, res, engine);
      res.end();
      return;
    }

    res.writeHead(404);
    res.end();
  };
}
