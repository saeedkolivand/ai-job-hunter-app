/**
 * Scraper sidecar HTTP protocol.
 *
 * All communication between the Tauri shell and this process goes through
 * HTTP POST /command → SSE ScraperEvent stream.
 *
 * These types mirror ScraperCommand / ScraperEvent in
 * apps/desktop/src/main/scraper-runtime.ts. They are duplicated here so the
 * sidecar remains standalone (no Electron / main-process imports).
 *
 * ── Request ──────────────────────────────────────────────────────────────────
 *  POST /command
 *  Content-Type: application/json
 *  Body: ScraperCommand
 *
 * ── Response ─────────────────────────────────────────────────────────────────
 *  Content-Type: text/event-stream
 *  Each SSE event is a JSON-serialised ScraperEvent:
 *    event: <kind>\ndata: <JSON>\n\n
 *
 * ── Health ────────────────────────────────────────────────────────────────────
 *  GET /health → 200 application/json ScraperRuntimeHealth
 *
 * ── Port announcement ─────────────────────────────────────────────────────────
 *  On startup the sidecar writes one line to stdout:
 *    {"port":<N>,"status":"ready"}\n
 *  The Tauri shell reads this to discover the sidecar's HTTP port.
 */

export type ScraperCommand =
  | { kind: 'scrape.board'; jobId: string; payload: ScrapeBoardPayload }
  | { kind: 'scrape.url'; jobId: string; payload: { url: string } }
  | { kind: 'cancel'; jobId: string }
  | { kind: 'set.credentials'; boardId: string; username: string; password: string }
  | { kind: 'open.login'; boardId: string }
  | { kind: 'board.status'; boardId: string }
  | { kind: 'board.disconnect'; boardId: string }
  | { kind: 'extract.text'; jobId: string; payload: { name: string; bytesBase64: string } }
  | { kind: 'apply.job'; jobId: string; payload: ApplyJobPayload }
  | { kind: 'apply.catalog' }
  | {
      kind: 'document.import';
      jobId: string;
      payload: { name: string; bytesBase64: string; locale?: string };
    }
  | { kind: 'document.list'; jobId: string }
  | { kind: 'document.remove'; jobId: string; payload: { id: string } }
  | {
      kind: 'search.hybrid';
      jobId: string;
      payload: { query: string; collection: string; topK?: number };
    }
  | { kind: 'match.resume'; jobId: string; payload: { resumeId: string; jobText: string } }
  | { kind: 'set.performance_mode'; mode: 'low-memory' | 'balanced' | 'performance' }
  | { kind: 'health' }
  | { kind: 'catalog' };

export type ScraperEvent =
  | { kind: 'progress'; jobId: string; p: number }
  | { kind: 'item'; jobId: string; item: Record<string, unknown> }
  | { kind: 'done'; jobId: string; result: unknown }
  | { kind: 'error'; jobId: string; message: string }
  | { kind: 'health.reply'; health: ScraperRuntimeHealth }
  | { kind: 'catalog.reply'; scrapers: ScraperCatalogEntry[] }
  | { kind: 'login.status'; boardId: string; connected: boolean; note?: string };

export interface ApplyJobPayload {
  board: string;
  url: string;
  coverLetter?: string;
  /** Base64-encoded resume bytes. The sidecar writes them to a temp file. */
  resumeBytesBase64?: string;
  resumeName?: string;
  autoSubmit?: boolean;
}

export interface ApplyResult {
  ok: boolean;
  stage: string;
  submitted: boolean;
  url: string;
  note?: string;
}

export interface ScrapeBoardPayload {
  board: string;
  query: string;
  location?: string;
  pages: number;
  dateFilter?: string;
  locale?: string;
}

export interface ScraperCatalogEntry {
  id: string;
  displayName: string;
  mode: 'http' | 'browser';
}

export interface ScraperRuntimeHealth {
  mode: 'http-sidecar';
  scrapers: ScraperCatalogEntry[];
  ready: boolean;
  port: number;
}

export interface PortAnnouncement {
  port: number;
  status: 'ready';
}
