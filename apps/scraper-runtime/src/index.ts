/**
 * Scraper sidecar entry point.
 *
 * Starts an HTTP server on a random localhost port and announces it on stdout:
 *   {"port":<N>,"status":"ready"}
 *
 * The Tauri shell reads this line and stores the port so subsequent scrape
 * commands can be proxied to this process over HTTP.
 *
 * Data directory:
 *   AJH_DATA_DIR env var   — explicit override (useful in tests)
 *   ~/.ajh                 — default
 *
 * Rollback:
 *   AJH_SCRAPER_MODE=in-process  — the Tauri shell falls back to stubs and
 *   does not launch this sidecar. See rollback-flags.ts in apps/desktop.
 */
import http from 'node:http';
import type { AddressInfo } from 'node:net';
import os from 'node:os';
import path from 'node:path';

import { FileCredentialStore } from './credentials.js';
import { ScraperEngine } from './engine.js';
import { createRequestHandler, setServerPort } from './server.js';

const host = '127.0.0.1';
const dataDir = process.env.AJH_DATA_DIR ?? path.join(os.homedir(), '.ajh');

const credentials = new FileCredentialStore(dataDir);
const engine = new ScraperEngine(credentials, dataDir);

// Open the vector/document store asynchronously — non-fatal if it fails.
void engine.openDataStore().catch((e: unknown) => {
  process.stderr.write(`[scraper-runtime] data store open warning: ${String(e)}\n`);
});

const server = http.createServer(createRequestHandler(engine));

server.listen(0, host, () => {
  const { port } = server.address() as AddressInfo;
  setServerPort(port);

  // Single JSON line on stdout — sidecar.rs parses this to discover the port.
  // Must be the first (and only) non-empty line on stdout before requests.
  process.stdout.write(JSON.stringify({ port, status: 'ready' }) + '\n');

  process.stderr.write(
    `[scraper-runtime] HTTP server listening on http://${host}:${port}\n` +
      `[scraper-runtime] data dir: ${dataDir}\n` +
      `[scraper-runtime] scrapers: ${engine.catalog().length} registered\n`
  );
});

// Graceful shutdown: cancel running jobs and close Playwright browser.
const shutdown = () => {
  process.stderr.write('[scraper-runtime] shutting down\n');
  server.close();
  void engine.shutdown().finally(() => process.exit(0));
};
process.on('SIGTERM', shutdown);
process.on('SIGINT', shutdown);
