import { type CatalogueEntry, fail, SHARD_LINE_BUDGET } from './model.js';

// ── Rust emission ───────────────────────────────────────────────────────────────────────────────

function rustStr(s: string): string {
  const escaped = s
    .replace(/\\/g, '\\\\')
    .replace(/"/g, '\\"')
    .replace(/\n/g, '\\n')
    .replace(/\r/g, '\\r')
    .replace(/\t/g, '\\t');
  return `"${escaped}"`;
}

function rustStrSlice(items: string[]): string {
  return items.length === 0 ? '&[]' : `&[${items.map(rustStr).join(', ')}]`;
}

/** `CatalogueArg.fields`'s Rust rendering — see that field's own TS doc for the three states.
 *  ponytail: an unresolved wrapper (`null`) and a resolved-but-empty one both render as
 *  `Some(&[])` — every real Zod/interface wrapper type in this codebase has at least one field
 *  (verified at generation time; `fail()` below would catch a future zero-field one going
 *  unnoticed), so this never actually collapses two live cases. Ceiling: if that stops being
 *  true, split into `Some(&[])` vs a dedicated `unresolved: bool` on `CatalogueArg`. */
function rustFieldsOption(fields: string[] | null | undefined): string {
  if (fields === undefined) return 'None';
  if (fields === null) return 'Some(&[])';
  if (fields.length === 0) {
    fail(
      'a wrapper type resolved to zero fields — the null/empty collapse in rustFieldsOption no longer holds'
    );
  }
  return `Some(${rustStrSlice(fields)})`;
}

/** One `CatalogueEntry` struct literal's own Rust lines — used both to RENDER a shard file and to
 *  MEASURE how many lines an entry costs while packing shards (`packByBudget`), so the two can
 *  never disagree about an entry's size. */
function renderEntryLines(entry: CatalogueEntry): string[] {
  const lines: string[] = ['    CatalogueEntry {'];
  lines.push(`        command: ${rustStr(entry.command)},`);
  lines.push(`        description: ${rustStr(entry.description)},`);
  if (entry.args.length === 0) {
    lines.push('        args: &[],');
  } else {
    lines.push('        args: &[');
    for (const arg of entry.args) {
      lines.push(
        `            CatalogueArg { name: ${rustStr(arg.name)}, required: ${arg.required}, fields: ${rustFieldsOption(arg.fields)} },`
      );
    }
    lines.push('        ],');
  }
  lines.push('    },');
  return lines;
}

/** One generated `catalogue/<name>.rs` file: every entry whose command starts `<prefix>_`. */
export interface Shard {
  prefix: string;
  /** File/module name: the prefix, or `<prefix>_2`, `_3`, … when one prefix spills over the budget. */
  name: string;
  entries: CatalogueEntry[];
}

/** Rust keywords a command prefix could collide with — such a module is declared `r#<name>`
 *  (its file stays `<name>.rs`). */
const RUST_KEYWORDS = new Set(
  (
    'as async await break const continue crate dyn else enum extern false fn for gen if impl in ' +
    'let loop match mod move mut pub ref return self static struct super trait true try type ' +
    'unsafe use where while abstract become box do final macro override priv typeof unsized ' +
    'virtual yield'
  ).split(' ')
);

function modIdent(name: string): string {
  return RUST_KEYWORDS.has(name) ? `r#${name}` : name;
}

/** One shard per command-name prefix (`ai_embed` → `ai.rs`), so each file is named for what it
 *  holds and stays stable as commands are added. `entries` is sorted, which keeps every prefix
 *  contiguous, so concatenating the shards in order reproduces the sorted table. A prefix whose
 *  entries exceed [`SHARD_LINE_BUDGET`] spills into `<prefix>_2`, `<prefix>_3`, … */
export function shardEntries(entries: CatalogueEntry[]): Shard[] {
  const byPrefix = new Map<string, CatalogueEntry[]>();
  for (const entry of entries) {
    const prefix = entry.command.split('_')[0] ?? '';
    if (!/^[a-z][a-z0-9]*$/.test(prefix)) {
      fail(`command \`${entry.command}\` has no usable snake_case prefix to name its shard by`);
    }
    byPrefix.set(prefix, [...(byPrefix.get(prefix) ?? []), entry]);
  }
  return [...byPrefix].flatMap(([prefix, group]) =>
    packByBudget(group).map((part, i) => ({
      prefix,
      name: i === 0 ? prefix : `${prefix}_${i + 1}`,
      entries: part,
    }))
  );
}

/** Greedily pack `entries` (already sorted) into parts, each capped at [`SHARD_LINE_BUDGET`]
 *  rendered lines — never a fixed shard COUNT, which would need bumping by hand as the catalogue
 *  grows, and never a fixed entries-per-shard count, which a few arg-heavy commands could blow
 *  past the LOC cap despite looking "even" by entry count. This is pure DATA (a `CatalogueEntry`/
 *  `CatalogueArg` struct literal), not logic that could be reorganized to fit this crate's own R8
 *  hard LOC cap (`docs/architecture-rules.md`) another way: rustfmt's own default `struct_lit_width`
 *  (18) is far below any `CatalogueEntry`'s own body length (`command`+`description`+`args` alone
 *  clear it), so every entry is forced one field per line no matter how short its own fields are —
 *  a single ~160-command file cannot fit under the cap at all. `struct_lit_width` does NOT govern
 *  a nested `CatalogueArg` the same way (short ones stay one line inside `args: &[...]`); those
 *  wrap instead when the rendered LINE exceeds `max_width` — the pre-rustfmt line count this
 *  function packs on can undercount that case, which is why the caller re-verifies the REAL
 *  rustfmt line count per shard (issue #1183 O1). Only reached for a single prefix too big for
 *  one file — [`shardEntries`] splits by prefix first. */
function packByBudget(entries: CatalogueEntry[]): CatalogueEntry[][] {
  const shards: CatalogueEntry[][] = [];
  let current: CatalogueEntry[] = [];
  let currentLines = 0;
  for (const entry of entries) {
    const entryLineCount = renderEntryLines(entry).length;
    if (current.length > 0 && currentLines + entryLineCount > SHARD_LINE_BUDGET) {
      shards.push(current);
      current = [];
      currentLines = 0;
    }
    current.push(entry);
    currentLines += entryLineCount;
  }
  if (current.length > 0) shards.push(current);
  return shards;
}

export function renderShardFile(shard: Shard): string {
  const body = shard.entries.flatMap(renderEntryLines);
  // A shard whose entries all take no arguments never names `CatalogueArg` (unused import).
  const imports = body.some((l) => l.includes('CatalogueArg'))
    ? '{CatalogueArg, CatalogueEntry}'
    : 'CatalogueEntry';
  return [
    '// @generated by `pnpm gen:agent-catalogue` — DO NOT EDIT BY HAND.',
    `// The \`${shard.prefix}_*\` commands of the sharded command catalogue — see`,
    "// `../catalogue.rs`'s own doc for why this table is split at all.",
    '',
    `use super::${imports};`,
    '',
    'pub(super) const ENTRIES: &[CatalogueEntry] = &[',
    ...body,
    '];',
    '',
  ].join('\n');
}

export function renderAggregator(
  shards: Shard[],
  totalEntries: number,
  uncataloguedNames: string[]
): string {
  return [
    '// @generated by `pnpm gen:agent-catalogue` — DO NOT EDIT BY HAND.',
    '// Source of truth: apps/desktop/src/tauri-client/namespaces/**/*.ts (argument shape)',
    '//   + packages/shared/src/ipc/contracts/*.ts (description).',
    '// Generator: packages/shared/scripts/gen-agent-catalogue.ts. Run `pnpm gen:agent-catalogue`.',
    '// CI runs `pnpm gen:agent-catalogue:check` to catch drift.',
    '//',
    `// ${totalEntries} commands catalogued across ${shards.length} file(s), one per command-name prefix (catalogue/<prefix>.rs),`,
    `// ${uncataloguedNames.length} uncatalogued (see UNCATALOGUED below).`,
    '//',
    "// Sharded rather than one big const array — this crate's own R8 hard LOC cap",
    "// (docs/architecture-rules.md) has no exception for generated DATA, and rustfmt's own default",
    '// `struct_lit_width` (18) forces every CatalogueEntry one field per line regardless of how',
    "// short its own fields are, so this file's size scales with the command count with no upper",
    '// bound this generator controls. A `LazyLock<Vec<_>>` — never a `const` array-concat, which',
    '// Rust cannot express across separately-compiled const items without an allocation — is',
    '// transparent to every call site: `Deref<Target = Vec<CatalogueEntry>>` ->',
    '// `Deref<Target = [CatalogueEntry]>` means `CATALOGUE.iter()`/`.find(...)` read exactly as',
    '// they would against a plain slice.',
    '',
    'use std::sync::LazyLock;',
    '',
    ...shards.map((shard) => `mod ${modIdent(shard.name)};`),
    '',
    '/// One declared argument of a [`CatalogueEntry`] — a top-level `--input`/`input` key exactly',
    '/// as the tauri-client sends it, whether it is required, and — for a wrapper key typed as a',
    "/// generated request struct or a plain contract interface — that type's own field names, so a",
    '/// nested unknown field is catchable too.',
    '///',
    '/// `fields` is three-state: `None` — not a wrapper key at all (a scalar arg). `Some(&[])` —',
    '/// a wrapper TYPE was identified but this generator could not resolve its field names (e.g.',
    '/// a rest-destructured request object); dispatch-time validation treats this the same as',
    '/// `None` (nothing to check a nested key against), but the `commands` MCP tool surfaces it',
    '/// as `"fields": null`, distinct from omitting the key entirely, so a caller can tell',
    '/// "known to take no nested fields" apart from "unknown nested shape". `Some([...])` —',
    "/// resolved: that type's own field names.",
    '#[derive(Clone, Copy)]',
    'pub(crate) struct CatalogueArg {',
    "    pub(crate) name: &'static str,",
    '    pub(crate) required: bool,',
    "    pub(crate) fields: Option<&'static [&'static str]>,",
    '}',
    '',
    "/// One dispatchable command's declared input contract (issues #1163, #1158, #1160).",
    '#[derive(Clone, Copy)]',
    'pub(crate) struct CatalogueEntry {',
    "    pub(crate) command: &'static str,",
    "    pub(crate) description: &'static str,",
    "    pub(crate) args: &'static [CatalogueArg],",
    '}',
    '',
    "/// The full catalogue, assembled from every shard's own `ENTRIES` at first access — see this",
    "/// file's own header comment for why a `LazyLock<Vec<_>>` and not a plain `const` slice.",
    'pub(crate) static CATALOGUE: LazyLock<Vec<CatalogueEntry>> = LazyLock::new(|| {',
    `    let mut entries = Vec::with_capacity(${totalEntries});`,
    ...shards.map((shard) => `    entries.extend_from_slice(${modIdent(shard.name)}::ENTRIES);`),
    '    entries',
    '});',
    '',
    '/// Commands with at least one `invoke()` call this generator could not parse with confidence',
    '/// (a computed key, a spread, a non-literal command name, or a non-object second argument) —',
    '/// listed rather than silently dropped. A command with NO `invoke()` call at all (zero',
    "/// renderer references — see `policy.rs`'s own module doc) is absent from here too; the",
    '/// coverage test pairs both arrays with a hand-written allowlist for exactly that case.',
    '// Read only from `#[cfg(test)]` code today (the coverage test above) — a plain, non-test',
    '// `cargo check --lib` sees no reader at all, so this stays legitimately unused OUTSIDE tests',
    "// (never a silenced real finding — the RUST equivalent of `policy.rs`'s own module-level",
    '// allow).',
    '#[allow(dead_code)]',
    'pub(crate) const UNCATALOGUED: &[&str] = &[',
    ...uncataloguedNames.map((name) => `    ${rustStr(name)},`),
    '];',
    '',
  ].join('\n');
}
