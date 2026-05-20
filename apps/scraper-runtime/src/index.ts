/**
 * Scraper sidecar entry point.
 *
 * Starts an HTTP server on a random localhost port and announces it on stdout:
 *   {"port":<N>,"status":"ready"}
 *
 * The Tauri shell reads this line and stores the port so subsequent scrape
 * commands can be proxied to this process over HTTP.
 *
 * Rollback:
 *   AJH_SCRAPER_MODE=in-process  — the Tauri shell falls back to stubs and
 *   does not launch this sidecar. See rollback-flags.ts in apps/desktop.
 */
import http from 'node:http';
import type { AddressInfo } from 'node:net';

import { createRequestHandler, setServerPort } from './server.js';

const host = '127.0.0.1';

const server = http.createServer(createRequestHandler());

server.listen(0, host, () => {
  const { port } = server.address() as AddressInfo;
  setServerPort(port);

  // Single JSON line on stdout — the Tauri sidecar.rs parses this to discover
  // our port. Must be the first (and only) line written before requests arrive.
  process.stdout.write(JSON.stringify({ port, status: 'ready' }) + '\n');

  // Structured log to stderr so it appears in diagnostic logs without
  // interfering with the stdout port-announcement protocol.
  process.stderr.write(`[scraper-runtime] HTTP server listening on http://${host}:${port}\n`);
});

// Graceful shutdown on SIGTERM/SIGINT.
const shutdown = () => {
  process.stderr.write('[scraper-runtime] shutting down\n');
  server.close(() => process.exit(0));
};
process.on('SIGTERM', shutdown);
process.on('SIGINT', shutdown);
