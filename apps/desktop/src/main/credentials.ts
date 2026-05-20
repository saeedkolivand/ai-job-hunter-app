/**
 * Encrypted credential store.
 *
 * - Secrets are encrypted with Electron's `safeStorage` (Keychain on macOS,
 *   DPAPI on Windows, libsecret on Linux). They are never written in plain
 *   text and never leave the main process — the renderer only ever sees
 *   metadata (boardId, username, savedAt).
 * - Storage file: <userData>/credentials.json
 *   Schema: { [boardId]: { username, secretB64, savedAt } }
 *
 * SECURITY NOTES:
 *  - `safeStorage` requires the user's OS account; if the file is copied to
 *    another machine, decryption will fail (intended).
 *  - We expose ONLY metadata to the renderer. The plaintext password is
 *    consumed inside the main process by scrapers.
 *  - Logging in to a third-party board may violate that platform's ToS.
 *    The UI surfaces this to the user before storing credentials.
 */
import { app, safeStorage } from 'electron';
import { readFile, writeFile, mkdir } from 'node:fs/promises';
import path from 'node:path';
import { createLogger } from '@ajh/core';

const logger = createLogger('credentials');

interface RawEntry {
  username: string;
  secretB64: string;
  savedAt: number;
}
interface StoreFile {
  version: 1;
  entries: Record<string, RawEntry>;
}

export interface CredentialMetadata {
  boardId: string;
  username: string;
  savedAt: number;
}

export interface DecryptedCredential {
  boardId: string;
  username: string;
  password: string;
}

const FILE_VERSION = 1;

export class CredentialStore {
  private cache?: StoreFile;
  private readonly file: string;

  constructor() {
    this.file = path.join(app.getPath('userData'), 'credentials.json');
  }

  isEncryptionAvailable(): boolean {
    return safeStorage.isEncryptionAvailable();
  }

  private async load(): Promise<StoreFile> {
    if (this.cache) return this.cache;
    try {
      const raw = await readFile(this.file, 'utf8');
      const parsed = JSON.parse(raw) as StoreFile;
      if (parsed.version !== FILE_VERSION) throw new Error('Unsupported version');
      this.cache = parsed;
    } catch {
      this.cache = { version: FILE_VERSION, entries: {} };
    }
    return this.cache;
  }

  private async persist(): Promise<void> {
    if (!this.cache) return;
    await mkdir(path.dirname(this.file), { recursive: true });
    await writeFile(this.file, JSON.stringify(this.cache, null, 2), { mode: 0o600 });
  }

  /** Metadata only — safe to expose to renderer. */
  async list(): Promise<CredentialMetadata[]> {
    const store = await this.load();
    return Object.entries(store.entries).map(([boardId, e]) => ({
      boardId,
      username: e.username,
      savedAt: e.savedAt,
    }));
  }

  async set(boardId: string, username: string, password: string): Promise<void> {
    if (!safeStorage.isEncryptionAvailable()) {
      throw new Error('OS encryption unavailable; refusing to store credentials in plain text');
    }
    const store = await this.load();
    const encrypted = safeStorage.encryptString(password);
    store.entries[boardId] = {
      username,
      secretB64: encrypted.toString('base64'),
      savedAt: Date.now(),
    };
    await this.persist();
    logger.info({ boardId, username }, 'credentials saved');
  }

  async remove(boardId: string): Promise<void> {
    const store = await this.load();
    if (store.entries[boardId]) {
      delete store.entries[boardId];
      await this.persist();
      logger.info({ boardId }, 'credentials removed');
    }
  }

  /** Main-process only. NEVER expose this through IPC. */
  async getDecrypted(boardId: string): Promise<DecryptedCredential | null> {
    const store = await this.load();
    const entry = store.entries[boardId];
    if (!entry) return null;
    try {
      const password = safeStorage.decryptString(Buffer.from(entry.secretB64, 'base64'));
      return { boardId, username: entry.username, password };
    } catch (err) {
      logger.error({ boardId, err }, 'failed to decrypt credentials (likely OS account mismatch)');
      return null;
    }
  }

  /** Path on disk where Playwright should persist this board's browser state. */
  storageStatePath(boardId: string): string {
    return path.join(app.getPath('userData'), 'browser-state', `${sanitize(boardId)}`);
  }
}

function sanitize(s: string): string {
  return s.replace(/[^a-z0-9_-]/gi, '_');
}
