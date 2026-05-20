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
  | { kind: 'health' }
  | { kind: 'catalog' };

export type ScraperEvent =
  | { kind: 'progress'; jobId: string; p: number }
  | { kind: 'item'; jobId: string; item: Record<string, unknown> }
  | { kind: 'done'; jobId: string; result: unknown }
  | { kind: 'error'; jobId: string; message: string }
  | { kind: 'health.reply'; health: ScraperRuntimeHealth }
  | { kind: 'catalog.reply'; scrapers: ScraperCatalogEntry[] };

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
