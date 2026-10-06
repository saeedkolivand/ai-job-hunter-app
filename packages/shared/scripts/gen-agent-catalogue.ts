/**
 * Agent-CLI command catalogue — the declared input contract every dispatchable command carries on
 * the generic `agent call`/MCP `call-*` tier (issues #1163, #1158, #1160).
 *
 * Two sources of truth, both already authoritative for something else, so this generator invents
 * no new one:
 *   - `apps/desktop/src/tauri-client/namespaces/**\/*.ts` — the `invoke('<cmd>', { ... })` call
 *     sites ARE the wire shape the app actually receives (shorthand/explicit top-level keys,
 *     required-ness read off the calling function's own parameter type).
 *   - `packages/shared/src/ipc/contracts/*.ts` — the TSDoc on the matching contract member, read
 *     via the SAME `docOf` `gen-api-docs.mjs` already uses for `docs/API.md` (imported rather than
 *     copied) but summarized by this file's OWN `catalogueSummarize`, not that file's `summarize`:
 *     that one cuts on the first `.` OR `:`, fine for a table cell sitting next to the full doc but
 *     content-free or backtick-unbalanced when the cut result is the entire description, as it is
 *     here — plus, for a wrapper key typed as a generated Zod request schema or a plain contract
 *     interface, that type's own field names.
 *
 * Emits `apps/desktop/src-tauri/src/extension_bridge/agent_cli/catalogue.rs` (the aggregator —
 * struct defs, the `CATALOGUE`/`UNCATALOGUED` consts) plus its sibling `catalogue/<prefix>.rs`
 * files (the actual entry data, one file per command-name prefix to stay under this crate's R8 hard LOC cap — see
 * `renderAggregator`'s own doc). Read by `agent_call.rs`'s dispatch-time key validation and by the
 * MCP `commands` tool (`args`/`description`). A construct this generator does not understand (a
 * computed key, a spread, a non-literal command name, a non-object second argument) is listed in
 * `UNCATALOGUED` rather than guessed at — ponytail: no attempt to resolve a dynamic key or a
 * renamed variable's shape.
 *
 * Run `pnpm gen:agent-catalogue` to regenerate, or `pnpm gen:agent-catalogue --check` to fail when
 * the committed output is stale (used in CI, same shape as `gen:ipc:check`).
 */
import { execFileSync } from 'node:child_process';
import {
  existsSync,
  mkdirSync,
  readdirSync,
  readFileSync,
  statSync,
  unlinkSync,
  writeFileSync,
} from 'node:fs';
import { basename, join } from 'node:path';

import { abs, parseContractFiles, repoPath } from '../../../scripts/gen-api-docs.mjs';
import { collectContractDescriptions } from './gen-agent-catalogue/describe.js';
import {
  collectContractInterfaceFields,
  collectScalarTypeAliasNames,
  collectZodTypeAliases,
  type FieldSources,
} from './gen-agent-catalogue/field-sources.js';
import { processNamespaceFile } from './gen-agent-catalogue/invoke-calls.js';
import {
  type CatalogueEntry,
  fail,
  OUT_FILE,
  REPO_ROOT,
  SHARD_DIR,
  SHARD_LINE_BUDGET,
  TAURI_CLIENT_DIR,
  type Uncatalogued,
} from './gen-agent-catalogue/model.js';
import { renderAggregator, renderShardFile, shardEntries } from './gen-agent-catalogue/render.js';

// Public surface of this module — `gen-agent-catalogue.test.ts` and the `.cli.ts` entry import these.
export { collectContractInterfaceFields } from './gen-agent-catalogue/field-sources.js';
export { mergeCatalogueEntry } from './gen-agent-catalogue/merge.js';
export type { CatalogueArg, CatalogueEntry } from './gen-agent-catalogue/model.js';
export { REPO_ROOT } from './gen-agent-catalogue/model.js';

export async function main() {
  const descCtx = collectContractDescriptions();

  const zodAliases = collectZodTypeAliases();
  const zodSchemas = (await import('../src/schemas/index.js')) as unknown as Record<
    string,
    unknown
  >;

  const contractSources = parseContractFiles();
  const interfaceFields = collectContractInterfaceFields(contractSources);
  const scalarTypeAliases = collectScalarTypeAliasNames();
  const fieldSources: FieldSources = { zodAliases, zodSchemas, interfaceFields, scalarTypeAliases };

  const entries = new Map<string, CatalogueEntry>();
  const descByNamespaceCommand = new Map<string, string>();
  const uncatalogued: Uncatalogued[] = [];

  // `.sort()` both listings — `readdirSync` order is filesystem-dependent (POSIX scandir order on
  // Linux CI, NTFS index order locally), and this loop's first-call-site-wins duplicate handling
  // (`mergeCatalogueEntry`) means an unsorted walk would let THAT incidental order decide which
  // namespace wins a duplicate command's description (SECURITY/MEDIUM, CLI review round 1).
  const nsDir = abs(TAURI_CLIENT_DIR);
  for (const dirName of readdirSync(nsDir).sort()) {
    const dirPath = join(nsDir, dirName);
    if (!statSync(dirPath).isDirectory()) continue;
    for (const f of readdirSync(dirPath).sort()) {
      if (f === 'index.ts' || f.endsWith('.test.ts') || !f.endsWith('.ts')) continue;
      processNamespaceFile(
        repoPath(join(dirPath, f)),
        descCtx,
        fieldSources,
        entries,
        descByNamespaceCommand,
        uncatalogued
      );
    }
  }

  const uncataloguedNames = [...new Set(uncatalogued.map((u) => u.command))].sort();
  for (const cmd of uncataloguedNames) entries.delete(cmd);

  const sortedEntries = [...entries.values()].sort((a, b) => a.command.localeCompare(b.command));
  const shards = shardEntries(sortedEntries);

  // `(repo-relative path, formatted content)` for the aggregator AND every shard — one list, so
  // the write/check loops below can never drift from what was actually rendered.
  const outputs: [string, string][] = [
    [
      OUT_FILE,
      formatWithRustfmt(renderAggregator(shards, sortedEntries.length, uncataloguedNames)),
    ],
  ];
  for (const shard of shards) {
    const formatted = formatWithRustfmt(renderShardFile(shard));
    // Issue #1183 O1: `shardEntries` packs on the PRE-rustfmt, entries-ONLY line count
    // (`renderEntryLines`'s own JS rendering, excluding the fixed header/`use`/footer
    // boilerplate `renderShardFile` wraps it in) — real rustfmt output can still exceed that
    // estimate for an entry rustfmt wraps FURTHER (a long `fields: Some(&[...])` slice literal on
    // one `CatalogueArg` line, past `max_width`). Re-derive the same entries-only count directly
    // from the REAL formatted lines strictly between the `ENTRIES` declaration and its closing
    // `];` (never a separate empty-shard baseline — rustfmt COLLAPSES `&[\n];` to `&[];` for a
    // genuinely empty array, which would silently overcount the header/footer by one line) so
    // this comparison is apples-to-apples with `SHARD_LINE_BUDGET`.
    const formattedLines = formatted.split('\n');
    const declLine = formattedLines.findIndex((l) => l.includes('const ENTRIES'));
    const closeLine = formattedLines.lastIndexOf('];');
    const entryLineCount = closeLine - declLine - 1;
    if (entryLineCount > SHARD_LINE_BUDGET) {
      fail(
        `${shard.name}.rs's entries render to ${entryLineCount} lines after rustfmt, over ` +
          `SHARD_LINE_BUDGET (${SHARD_LINE_BUDGET}) — rustfmt wrapped an entry further than the ` +
          `pre-format estimate; lower SHARD_LINE_BUDGET or shrink the offending entry's rendering.`
      );
    }
    outputs.push([join(SHARD_DIR, `${shard.name}.rs`), formatted]);
  }

  // Shard files this run no longer produces (a prefix disappeared, or the old `shard_N.rs`
  // naming) — deleted rather than left as stale, since a `mod` that no longer exists in the
  // aggregator would otherwise leave an orphaned, unreferenced file behind forever. The directory
  // holds generated shards only.
  const shardDirAbs = abs(SHARD_DIR);
  const currentShardFileNames = new Set(outputs.map(([p]) => basename(p)));
  const staleShardFiles = existsSync(shardDirAbs)
    ? readdirSync(shardDirAbs).filter((f) => f.endsWith('.rs') && !currentShardFileNames.has(f))
    : [];

  const check = process.argv.includes('--check');
  if (check) {
    let stale = staleShardFiles.length > 0;
    for (const [relPath, formatted] of outputs) {
      const target = join(REPO_ROOT, relPath);
      let current = '';
      try {
        current = readFileSync(target, 'utf8');
      } catch {
        // file doesn't exist yet — current stays ''
      }
      if (current !== formatted) stale = true;
    }
    if (stale) {
      console.error(`✗ stale: ${OUT_FILE} (or its shards) — run \`pnpm gen:agent-catalogue\``);
      process.exit(1);
    }
    console.log(
      `✓ ${OUT_FILE} + ${shards.length} shard(s) are up to date (${sortedEntries.length} commands catalogued)`
    );
    return;
  }

  mkdirSync(shardDirAbs, { recursive: true });
  for (const staleFile of staleShardFiles) unlinkSync(join(shardDirAbs, staleFile));
  for (const [relPath, formatted] of outputs) {
    writeFileSync(join(REPO_ROOT, relPath), formatted);
  }
  console.log(
    `✓ wrote ${OUT_FILE} + ${shards.length} shard(s) — ${sortedEntries.length} commands catalogued, ` +
      `${uncataloguedNames.length} uncatalogued`
  );
  if (uncatalogued.length > 0) {
    for (const u of uncatalogued) console.log(`  uncatalogued: ${u.command} — ${u.reason}`);
  }
}

/** Shells out to the REAL `rustfmt` — deliberately not a hand-rolled approximation of its
 *  wrapping heuristics the way `gen-ipc-rust.ts`'s array emitter is (that file's own doc walks
 *  through why: verified-against-one-version heuristics for a handful of primitive-array shapes).
 *  This generator's struct-literal nesting (`CatalogueEntry` containing a `CatalogueArg` slice)
 *  is a shape rustfmt's own struct-literal/array heuristics interact on, and getting that
 *  interaction wrong silently would mean a correctly-run `gen:agent-catalogue` still failing its
 *  own `--check` after `cargo fmt --all` — worse than the one extra process spawn this costs.
 *  `pnpm gen:agent-catalogue:check` (CI) installs the `rustfmt` component before calling this;
 *  local dev already has it via the pinned toolchain — but only if rustup actually SELECTS that
 *  pin, which it does off `rust-toolchain.toml`'s directory, not this script's cwd (this file
 *  runs under `packages/shared` via `pnpm --filter`). `cwd` below points the rustup proxy at
 *  `apps/desktop/src-tauri`, where that file lives, so this always resolves the SAME rustfmt
 *  `cargo fmt --check` gates on rather than the machine's/runner's default `stable`. */
function formatWithRustfmt(source: string): string {
  return execFileSync('rustfmt', ['--edition', '2024', '--emit', 'stdout'], {
    input: source,
    encoding: 'utf8',
    cwd: abs('apps/desktop/src-tauri'),
  });
}

// `main` is exported, never self-invoked here, so a test can `import` this module's pure helpers
// (e.g. `collectContractInterfaceFields`) without ALSO regenerating (and overwriting) the real
// `catalogue.rs`/shard files as a side effect of running the test suite. Direct invocation runs
// through the tiny `gen-agent-catalogue.cli.ts` wrapper instead (A1-r3-AC-3 MEDIUM — the former
// guard here compared `resolve(process.argv[1])` against `fileURLToPath(import.meta.url)` and
// silently did NOTHING if that ever stopped matching, which would make `pnpm
// gen:agent-catalogue:check` pass against a stale catalogue with nothing distinguishing "unchanged
// because up to date" from "unchanged because main() never ran" — the exact class of gap
// `gen-api-docs.cli.mjs` was already split out to close). A separate entry point has no such
// comparison to get wrong: it either runs or the process fails to even start.
