/**
 * LanceDB-backed vector store.
 *
 * Embedding dimensions are NOT hardcoded — each collection's schema is
 * created lazily on first insert, derived from the inserted vector size.
 * This keeps the store model-agnostic across bge-m3 / e5 / nomic / etc.
 */
import { connect, type Connection, type Table } from '@lancedb/lancedb';

import { createLogger } from '@ajh/core';

import type { CollectionName, VectorRecord } from './collections.js';

export interface SearchOptions {
  topK?: number;
  filter?: string; // LanceDB SQL-like predicate
}

export class VectorStore {
  private conn?: Connection;
  private readonly tables = new Map<CollectionName, Table>();
  private readonly logger = createLogger('vector-store');

  constructor(private readonly path: string) {}

  async open(): Promise<void> {
    this.conn = await connect(this.path);
    this.logger.info({ path: this.path }, 'lancedb opened');
  }

  async close(): Promise<void> {
    this.tables.clear();
    this.conn = undefined;
  }

  private async getOrCreateTable(name: CollectionName, sample: VectorRecord): Promise<Table> {
    if (!this.conn) throw new Error('VectorStore not open');
    const cached = this.tables.get(name);
    if (cached) return cached;
    const existing = await this.conn.tableNames();
    let table: Table;
    if (existing.includes(name)) {
      table = await this.conn.openTable(name);
    } else {
      table = await this.conn.createTable(name, [sample]);
      this.logger.info({ collection: name, dim: sample.vector.length }, 'created collection');
    }
    this.tables.set(name, table);
    return table;
  }

  async upsert(name: CollectionName, records: VectorRecord[]): Promise<void> {
    if (records.length === 0) return;
    const firstRecord = records[0];
    if (!firstRecord) return;
    const table = await this.getOrCreateTable(name, firstRecord);
    await table.add(records);
  }

  async search(
    name: CollectionName,
    queryVector: number[],
    opts: SearchOptions = {}
  ): Promise<Array<VectorRecord & { _distance: number }>> {
    if (!this.conn) throw new Error('VectorStore not open');
    const existing = await this.conn.tableNames();
    if (!existing.includes(name)) return [];
    const table = this.tables.get(name) ?? (await this.conn.openTable(name));
    this.tables.set(name, table);
    let q = table.search(queryVector).limit(opts.topK ?? 20);
    if (opts.filter) q = q.where(opts.filter);
    return (await q.toArray()) as Array<VectorRecord & { _distance: number }>;
  }

  async remove(name: CollectionName, ids: string[]): Promise<void> {
    if (!this.conn || ids.length === 0) return;
    const existing = await this.conn.tableNames();
    if (!existing.includes(name)) return;
    const table = this.tables.get(name) ?? (await this.conn.openTable(name));
    this.tables.set(name, table);
    const list = ids.map((id) => `'${id.replace(/'/g, "''")}'`).join(',');
    await table.delete(`id IN (${list})`);
  }
}
