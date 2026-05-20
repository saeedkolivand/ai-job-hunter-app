/**
 * FileCredentialStore — implements CredentialsAccessor without Electron.
 *
 * Phase A: credentials stored in plaintext JSON at <dataDir>/credentials.json.
 * Phase B (Tauri stronghold): the Tauri shell will encrypt at rest and push
 * decrypted pairs to the sidecar at startup via the set.credentials command.
 *
 * The store is also populated at runtime when the Tauri shell sends a
 * `set.credentials` command, keeping the in-memory copy current without
 * requiring a sidecar restart.
 *
 * ── Credential file format ───────────────────────────────────────────────────
 *  {
 *    "<boardId>": { "username": "...", "password": "..." },
 *    ...
 *  }
 *
 * ── storageStatePath ─────────────────────────────────────────────────────────
 *  <dataDir>/browser-state/<boardId>/
 *  Playwright uses this directory for persistent cookies / storage state,
 *  replacing Electron's `session.fromPartition("persist:<boardId>")`.
 */
import fs from 'node:fs';
import path from 'node:path';

import type { CredentialsAccessor } from '@ajh/data';

type CredEntry = { username: string; password: string };
type CredMap = Record<string, CredEntry>;

export class FileCredentialStore implements CredentialsAccessor {
  private readonly credFile: string;
  private readonly stateDir: string;
  /** In-memory overlay — set.credentials writes here and takes priority. */
  private overlay: CredMap = {};

  constructor(dataDir: string) {
    this.credFile = path.join(dataDir, 'credentials.json');
    this.stateDir = path.join(dataDir, 'browser-state');
    fs.mkdirSync(this.stateDir, { recursive: true });
  }

  async get(boardId: string): Promise<{ username: string; password: string } | null> {
    // Overlay (runtime push from Tauri shell) takes priority.
    if (this.overlay[boardId]) return this.overlay[boardId];

    // Fall back to credential file.
    try {
      const raw = fs.readFileSync(this.credFile, 'utf8');
      const map = JSON.parse(raw) as CredMap;
      return map[boardId] ?? null;
    } catch {
      return null;
    }
  }

  storageStatePath(boardId: string): string {
    const dir = path.join(this.stateDir, boardId);
    fs.mkdirSync(dir, { recursive: true });
    return dir;
  }

  /** Called by the set.credentials command handler. */
  set(boardId: string, username: string, password: string): void {
    this.overlay[boardId] = { username, password };
  }

  /** Persist the overlay to the credential file. */
  flush(): void {
    let existing: CredMap = {};
    try {
      existing = JSON.parse(fs.readFileSync(this.credFile, 'utf8')) as CredMap;
    } catch {
      /* first write */
    }
    const merged = { ...existing, ...this.overlay };
    fs.mkdirSync(path.dirname(this.credFile), { recursive: true });
    fs.writeFileSync(this.credFile, JSON.stringify(merged, null, 2), 'utf8');
  }
}
