import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

import { fail as apiFail } from '../../../../scripts/gen-api-docs.mjs';

export const REPO_ROOT = resolve(dirname(fileURLToPath(import.meta.url)), '../../../..');
export const TAURI_CLIENT_DIR = 'apps/desktop/src/tauri-client/namespaces';
/** Every `export type X = z.infer<typeof Y>` alias lives in one of the `schemas/*.ts` modules. */
export const SCHEMAS_DIR = 'packages/shared/src/schemas';
export const OUT_FILE = 'apps/desktop/src-tauri/src/extension_bridge/agent_cli/catalogue.rs';
// Sharded (see `renderShardFile`'s own doc for why): the aggregator's sibling `catalogue/`
// directory, matching this repo's `foo.rs` + `foo/*.rs` submodule-file convention.
export const SHARD_DIR = 'apps/desktop/src-tauri/src/extension_bridge/agent_cli/catalogue';
/** Rendered LOC per shard this generator targets — see `shardEntries`'s own doc. Leaves headroom
 *  under R8's `HARD_CAP_LOC` in `apps/desktop/src-tauri/tests/architecture.rs` for the shard
 *  header and rustfmt's line count. */
export const SHARD_LINE_BUDGET = 260;

export function fail(message: string): never {
  apiFail(`gen:agent-catalogue — ${message}`);
  throw new Error('unreachable'); // apiFail always throws; satisfies TS's `never` inference.
}

export interface CatalogueArg {
  name: string;
  required: boolean;
  /** `undefined` — not a wrapper key (no resolvable type at all): the arg is a scalar.
   *  `null` — a wrapper TYPE was identified but this generator could not resolve its field
   *  names (e.g. a rest-destructured request object, `findParamBinding`'s own documented gap).
   *  `string[]` — resolved: that type's own field names. Rendered as Rust `Option<&[&str]>` so
   *  the `commands` tool (and dispatch-time validation) can tell "known to take no nested
   *  fields" apart from "unknown nested shape" instead of collapsing both to an empty slice. */
  fields: string[] | null | undefined;
}

export interface CatalogueEntry {
  command: string;
  description: string;
  args: CatalogueArg[];
}

export type Uncatalogued = { command: string; reason: string };
