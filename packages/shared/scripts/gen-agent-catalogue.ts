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
 * struct defs, the `CATALOGUE`/`UNCATALOGUED` consts) plus its sibling `catalogue/shard_*.rs`
 * files (the actual entry data, split to stay under this crate's R8 hard LOC cap — see
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
import { basename, dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

import { z } from 'zod';

// Reused, never copied — the exact TSDoc-extraction/summary logic `docs/API.md` is built from.
// `ts` ITSELF is reused from here too, not imported bare (`import ts from 'typescript'`):
// this file lives under `packages/shared/scripts/`, whose nearest `typescript` devDependency is
// pinned to v7 (no classic Compiler API — see `gen-api-docs.mjs`'s own re-export comment), while
// `gen-api-docs.mjs` lives at the repo root, where the classic-API v6 line is pinned. Node/tsx
// resolve a bare specifier by the IMPORTING FILE's own location, so a second `import ts from
// 'typescript'` here would silently resolve the WRONG, incompatible package.
import {
  abs,
  docOf,
  fail as apiFail,
  INDEX_FILE,
  isExported,
  namespaceMap,
  parseContractFiles,
  repoPath,
  ts,
} from '../../../scripts/gen-api-docs.mjs';

const HERE = dirname(fileURLToPath(import.meta.url));
const REPO_ROOT = resolve(HERE, '../../..');
const TAURI_CLIENT_DIR = 'apps/desktop/src/tauri-client/namespaces';
const SCHEMAS_INDEX = 'packages/shared/src/schemas/index.ts';
const OUT_FILE = 'apps/desktop/src-tauri/src/extension_bridge/agent_cli/catalogue.rs';
// Sharded (see `renderShardFile`'s own doc for why): the aggregator's sibling `catalogue/`
// directory, matching this repo's `foo.rs` + `foo/*.rs` submodule-file convention.
const SHARD_DIR = 'apps/desktop/src-tauri/src/extension_bridge/agent_cli/catalogue';
/** Rendered LOC per shard this generator targets — see `shardEntries`'s own doc. Comfortably
 *  under `docs/architecture-rules.md`'s R8 hard cap (1400) even as the catalogue grows. */
const SHARD_LINE_BUDGET = 500;

function fail(message: string): never {
  apiFail(`gen:agent-catalogue — ${message}`);
  throw new Error('unreachable'); // apiFail always throws; satisfies TS's `never` inference.
}

// ── Contract descriptions (namespace + method -> first TSDoc sentence) ────────────────────────

interface DescCtx {
  namespaces: Map<string, string>;
  decls: Map<string, { file: string; sf: ts.SourceFile; node: ts.Node }>;
}

function collectContractDescriptions(): DescCtx {
  const sources = parseContractFiles();
  const indexSf = sources.get(repoPath(INDEX_FILE));
  if (!indexSf) fail(`${repoPath(INDEX_FILE)} not found`);
  const namespaces = namespaceMap(indexSf);
  const decls = new Map<string, { file: string; sf: ts.SourceFile; node: ts.Node }>();
  for (const [file, sf] of sources) {
    if (file === repoPath(INDEX_FILE)) continue;
    for (const stmt of sf.statements) {
      if (
        (ts.isInterfaceDeclaration(stmt) || ts.isTypeAliasDeclaration(stmt)) &&
        isExported(stmt)
      ) {
        decls.set(stmt.name.text, { file, sf, node: stmt });
      }
    }
  }
  return { namespaces, decls };
}

/** Below this length a `.`-cut sentence is more likely an abbreviation ("e.g.") or a mid-sentence
 *  fragment than a complete description — the full first paragraph is more informative here, where
 *  (unlike `docs/API.md`'s table cell) the cut result is the entire text an LLM ever sees. */
const MIN_CATALOGUE_DESCRIPTION_LENGTH = 40;

/** `true` when `text` has an odd number of backticks — a `.`-cut can still land inside a backtick
 *  span (e.g. a file extension: "see `foo.rs`.") and leave it unbalanced. */
function hasUnbalancedBacktick(text: string): boolean {
  return (text.match(/`/g)?.length ?? 0) % 2 === 1;
}

/** `true` when `text` has more `(` than `)` — a `.`-cut lands mid-abbreviation ("e.g.", "i.e.")
 *  more often than mid-backtick-span, and an abbreviation inside a parenthetical is this repo's
 *  own TSDoc style, so this is the more common of the two unbalanced-cut shapes in practice. */
function hasUnbalancedParen(text: string): boolean {
  return (text.match(/\(/g)?.length ?? 0) > (text.match(/\)/g)?.length ?? 0);
}

/** Matches one `.`-terminated sentence at the START of its input, sentence text in group 1. */
const SENTENCE_RE = /^(.*?\.)(\s|$)/;

/** One-line command description. Deliberately NOT `gen-api-docs.mjs`'s `summarize` (this generator
 *  reuses that file's `docOf`/parsing, never its summary): that function cuts on the first `.` OR
 *  `:`, correct for a `docs/API.md` table cell sitting next to the full doc, but wrong here, where
 *  the cut result IS the whole description — a colon-terminated fragment like "Factory reset:", a
 *  cut landing mid-abbreviation inside a parenthetical ("(e.g."), or a cut landing inside a
 *  backtick span otherwise reaches an LLM with no other source of truth. Cuts on `.` only. A short
 *  or unbalanced first sentence pulls in the NEXT sentence rather than falling back to the whole
 *  first paragraph (CLI review round 2 — MEDIUM: a 21-char but complete first sentence like "Run
 *  an autopilot now." used to publish a 275-char paragraph, and a 38-char one a 1096-char
 *  implementation-detail dump). Only a paragraph with no sentence boundary at all — the cut regex
 *  never matches — falls back to the full paragraph, since there is nothing shorter to extend. */
function catalogueSummarize(doc: string): string {
  if (!doc) return '';
  const firstPara = doc
    .split(/\n\s*\n/)[0]
    .replace(/\s*\n\s*/g, ' ')
    .trim();
  const firstMatch = SENTENCE_RE.exec(firstPara);
  if (!firstMatch) return firstPara;

  let cut = firstMatch[1].trim();
  let rest = firstPara.slice(firstMatch[0].length);
  while (
    (cut.length < MIN_CATALOGUE_DESCRIPTION_LENGTH ||
      hasUnbalancedBacktick(cut) ||
      hasUnbalancedParen(cut)) &&
    rest.length > 0
  ) {
    const nextMatch = SENTENCE_RE.exec(rest);
    if (!nextMatch) return firstPara; // no further sentence boundary — nothing shorter to use
    cut = `${cut} ${nextMatch[1].trim()}`;
    rest = rest.slice(nextMatch[0].length);
  }
  return cut;
}

/** First TSDoc sentence for `<namespace>.<method>`, or `''` when there is none to find. */
function describe(ctx: DescCtx, namespace: string, method: string): string {
  const contractName = ctx.namespaces.get(namespace);
  if (!contractName) return '';
  const contract = ctx.decls.get(contractName);
  if (!contract || !ts.isInterfaceDeclaration(contract.node)) return '';
  const member = contract.node.members.find((m) => m.name?.getText(contract.sf) === method);
  if (!member) return '';
  return catalogueSummarize(docOf(member, contract.sf));
}

// ── Nested field names for a wrapper key's type ────────────────────────────────────────────────

/** `TypeName -> SchemaConstName`, read from every `export type X = z.infer<typeof Y>;` in
 *  `packages/shared/src/schemas/index.ts` — the one naming convention every generated Zod request
 *  type in this repo follows (verified: 25/25 `z.infer` type aliases there match it). */
function collectZodTypeAliases(sf: ts.SourceFile): Map<string, string> {
  const map = new Map<string, string>();
  for (const stmt of sf.statements) {
    if (!ts.isTypeAliasDeclaration(stmt) || !isExported(stmt)) continue;
    const t = stmt.type;
    if (!ts.isTypeReferenceNode(t) || t.typeName.getText(sf) !== 'z.infer') continue;
    const arg = t.typeArguments?.[0];
    if (!arg || !ts.isTypeQueryNode(arg)) continue;
    map.set(stmt.name.text, arg.exprName.getText(sf));
  }
  return map;
}

/** Flat top-level member names of every exported interface declared directly in an
 *  `ipc/contracts/*.ts` file (never Zod-derived) — the second convention a wrapper key's type can
 *  follow, e.g. `BaseExportRequest`/`TemplateRecommendSignals`. */
function collectContractInterfaceFields(
  sources: Map<string, ts.SourceFile>
): Map<string, string[]> {
  const map = new Map<string, string[]>();
  for (const [, sf] of sources) {
    for (const stmt of sf.statements) {
      if (!ts.isInterfaceDeclaration(stmt) || !isExported(stmt)) continue;
      const fields = stmt.members.filter(ts.isPropertySignature).map((m) => m.name.getText(sf));
      map.set(stmt.name.text, fields);
    }
  }
  return map;
}

interface FieldSources {
  zodAliases: Map<string, string>;
  zodSchemas: Record<string, unknown>;
  interfaceFields: Map<string, string[]>;
  scalarTypeAliases: Set<string>;
}

/** Nested field names for a wrapper key typed as `typeName`, or `undefined` when this generator
 *  cannot resolve that type's shape (not an error — see the module doc: only a top-level construct
 *  it cannot parse at all is fatal). */
function resolveNestedFields(sources: FieldSources, typeName: string): string[] | undefined {
  const schemaName = sources.zodAliases.get(typeName);
  if (schemaName) {
    const schema = sources.zodSchemas[schemaName];
    if (schema instanceof z.ZodObject) {
      return Object.keys(schema.shape);
    }
  }
  return sources.interfaceFields.get(typeName);
}

// ── Scalar type aliases — A1-r1-AC-3 MEDIUM ────────────────────────────────────────────────────
//
// A wrapper key's named type can resolve to neither of the two sources above (e.g. `PipelineStage`,
// declared in `events/pipeline.ts` as `(typeof PIPELINE_STAGES)[number]`) while still genuinely
// being a SCALAR, not an object — `resolveNestedFields` returning `undefined` for it used to be
// read by `parseInvokeCall` as "a wrapper type was named but its shape is unresolved" (`fields:
// null`), publishing a plain string param as an object on the wire. This is a narrow THIRD lookup
// for exactly that one question ("is this declaration provably a scalar?"), never a general
// nested-field resolver — a positive answer here still yields `fields: undefined` (no nested
// fields to check), never a new resolved `Some([...])`; a negative or unknown answer keeps the
// existing conservative `null` default (a genuine unresolved object, e.g. `ResumePipelineRunRequest`
// — an `Omit<...> & ...` intersection — or `PerformanceBackendConfig` — an interface declared
// outside `ipc/contracts/*.ts` — must NOT flip to `undefined`, or the nested-shape signal is lost
// the other way).

/** Every non-test `.ts` file under `dir`, recursively. */
function listTsFiles(dir: string, out: string[] = []): string[] {
  for (const entry of readdirSync(dir)) {
    const full = join(dir, entry);
    if (statSync(full).isDirectory()) {
      listTsFiles(full, out);
    } else if (entry.endsWith('.ts') && !entry.endsWith('.test.ts')) {
      out.push(full);
    }
  }
  return out;
}

/** `true` for a string/number/boolean literal type, or its matching keyword — the atomic member
 *  shape both a scalar union and a `[...] as const` array element must have. */
function isScalarLiteralOrKeyword(type: ts.TypeNode): boolean {
  return (
    type.kind === ts.SyntaxKind.StringKeyword ||
    type.kind === ts.SyntaxKind.NumberKeyword ||
    type.kind === ts.SyntaxKind.BooleanKeyword ||
    (ts.isLiteralTypeNode(type) &&
      (ts.isStringLiteral(type.literal) ||
        ts.isNumericLiteral(type.literal) ||
        type.literal.kind === ts.SyntaxKind.TrueKeyword ||
        type.literal.kind === ts.SyntaxKind.FalseKeyword))
  );
}

/** `true` for a same-file `const`'s initializer (an `as const` array literal unwrapped) holding
 *  only string/number literal elements — the `PIPELINE_STAGES = [...] as const` shape a
 *  `(typeof X)[number]` alias indexes into. */
function isScalarConstArray(init: ts.Expression): boolean {
  const unwrapped = ts.isAsExpression(init) ? init.expression : init;
  return (
    ts.isArrayLiteralExpression(unwrapped) &&
    unwrapped.elements.every((el) => ts.isStringLiteral(el) || ts.isNumericLiteral(el))
  );
}

/** Strips a `TypeNode`'s wrapping parentheses — `(typeof X)[number]` parses `typeof X` as a
 *  `ParenthesizedTypeNode`, not a bare `TypeQueryNode`. */
function unwrapParens(type: ts.TypeNode): ts.TypeNode {
  return ts.isParenthesizedTypeNode(type) ? unwrapParens(type.type) : type;
}

/** `true` when `type` provably denotes a SCALAR shape: a keyword, a union of literals/keywords, or
 *  `(typeof CONST)[number]` over a same-file `const` array of literals. Anything else (an object
 *  type, an intersection, a generic, a reference this fn does not walk into) returns `false` —
 *  unproven, never guessed. */
function isScalarTypeNode(sf: ts.SourceFile, type: ts.TypeNode): boolean {
  if (isScalarLiteralOrKeyword(type)) return true;
  if (ts.isUnionTypeNode(type)) return type.types.every((t) => isScalarLiteralOrKeyword(t));
  if (!ts.isIndexedAccessTypeNode(type) || type.indexType.kind !== ts.SyntaxKind.NumberKeyword) {
    return false;
  }
  const objectType = unwrapParens(type.objectType);
  if (!ts.isTypeQueryNode(objectType) || !ts.isIdentifier(objectType.exprName)) return false;
  const constName = objectType.exprName.text;
  for (const stmt of sf.statements) {
    if (!ts.isVariableStatement(stmt)) continue;
    const decl = stmt.declarationList.declarations.find(
      (d) => ts.isIdentifier(d.name) && d.name.text === constName
    );
    if (decl?.initializer) return isScalarConstArray(decl.initializer);
  }
  return false;
}

/** Every EXPORTED type alias name across `packages/shared/src` (recursive, tests excluded) whose
 *  own declaration is a [`isScalarTypeNode`] shape — computed once in `main`, before any namespace
 *  file is parsed. */
function collectScalarTypeAliasNames(): Set<string> {
  const names = new Set<string>();
  for (const file of listTsFiles(abs('packages/shared/src'))) {
    const text = readFileSync(file, 'utf8');
    const sf = ts.createSourceFile(file, text, ts.ScriptTarget.Latest, true);
    for (const stmt of sf.statements) {
      if (!ts.isTypeAliasDeclaration(stmt) || !isExported(stmt)) continue;
      if (isScalarTypeNode(sf, stmt.type)) names.add(stmt.name.text);
    }
  }
  return names;
}

// ── Parsing the tauri-client invoke() call sites ───────────────────────────────────────────────

interface CatalogueArg {
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

interface CatalogueEntry {
  command: string;
  description: string;
  args: CatalogueArg[];
}

/** `true` for a union type annotation with an `undefined`/`null` member (`T | undefined`) — this
 *  repo's other idiom for "optional", alongside `?`/a default, e.g. `job_preferences.ts`'s
 *  `setSalaryExpectation: (salaryExpectation: string | undefined) => ...`. `JSON.stringify` drops
 *  an `undefined`-valued property, so the renderer's own clear-a-value call site never sends this
 *  key at all — treating it as required would refuse that call site's own payload. */
function isOptionalUnion(type: ts.TypeNode | undefined): boolean {
  return (
    !!type &&
    ts.isUnionTypeNode(type) &&
    type.types.some(
      (t) =>
        (ts.isLiteralTypeNode(t) && t.literal.kind === ts.SyntaxKind.NullKeyword) ||
        t.kind === ts.SyntaxKind.UndefinedKeyword
    )
  );
}

/** Sentinel `typeName` for a binding this generator knows IS an object wrapper but cannot resolve
 *  the field names of — never a real declared type name (angle brackets can't appear in a TS
 *  identifier), so `resolveNestedFields` naturally fails to look it up and the caller's `?? null`
 *  marks the arg a KNOWN, unresolved wrapper rather than an untyped scalar. Covers a
 *  rest-destructured binding (`{ id, ...data }`) AND a plain identifier param typed `unknown`, an
 *  inline object type literal, or an indexed-access type (`Parameters<Fn>[0]`) — none of those are
 *  a `TypeReferenceNode` this generator can look a name up for, but all four are genuinely a
 *  wrapper, not a scalar (A1-r1-AC-1 MEDIUM: these used to fall through to `fields: undefined`,
 *  publishing a real object wrapper on the wire as a plain scalar with no `fields` key at all — the
 *  exact "unknown nested shape" signal `fields: null` exists to carry). See `findParamBinding`'s
 *  own doc. */
const UNRESOLVED_WRAPPER_TYPE = '<unresolved-wrapper>';

/** `true` for a type-annotation shape this generator knows is an object wrapper but does not (yet)
 *  resolve field names for — see [`UNRESOLVED_WRAPPER_TYPE`]'s own doc. */
function isUnresolvableWrapperType(type: ts.TypeNode): boolean {
  return (
    type.kind === ts.SyntaxKind.UnknownKeyword ||
    ts.isTypeLiteralNode(type) ||
    ts.isIndexedAccessTypeNode(type)
  );
}

/** Every parameter of `fn` (both a plain identifier and a destructured `{ ... }` one) that could
 *  bind `name`, paired with what its OWN type annotation says about it. */
function findParamBinding(
  fn: ts.FunctionLikeDeclarationBase,
  name: string
): { questionOrDefault: boolean; typeName: string | undefined } | undefined {
  for (const param of fn.parameters) {
    if (ts.isIdentifier(param.name) && param.name.text === name) {
      const typeName =
        param.type && ts.isTypeReferenceNode(param.type) && param.type.typeArguments === undefined
          ? param.type.typeName.getText()
          : param.type && isUnresolvableWrapperType(param.type)
            ? UNRESOLVED_WRAPPER_TYPE
            : undefined;
      return {
        questionOrDefault:
          Boolean(param.questionToken) || Boolean(param.initializer) || isOptionalUnion(param.type),
        typeName,
      };
    }
    if (ts.isObjectBindingPattern(param.name)) {
      const el = param.name.elements.find(
        (e) => ts.isIdentifier(e.name) && e.name.text === name && !e.dotDotDotToken
      );
      if (!el) {
        // Does `name` bind the REST element instead (`{ id, ...data }`)? Its shape is "the
        // param's own type minus the named siblings", which this generator does not compute —
        // but it IS a wrapper, so the resulting arg should read `fields: null` (unresolved),
        // not `fields: undefined` (scalar) — issue #1158's "guess the wrapper" gap, CLI review
        // round 1 (MEDIUM).
        const restEl = param.name.elements.find(
          (e) => e.dotDotDotToken && ts.isIdentifier(e.name) && e.name.text === name
        );
        if (restEl) {
          return { questionOrDefault: false, typeName: UNRESOLVED_WRAPPER_TYPE };
        }
        continue;
      }
      const propName = (el.propertyName ?? el.name) as ts.Identifier;
      if (el.initializer) {
        return { questionOrDefault: true, typeName: undefined };
      }
      if (param.type && ts.isTypeLiteralNode(param.type)) {
        const member = param.type.members.find(
          (m) => ts.isPropertySignature(m) && m.name.getText() === propName.text
        ) as ts.PropertySignature | undefined;
        const memberType = member?.type;
        const typeName =
          memberType && ts.isTypeReferenceNode(memberType) && memberType.typeArguments === undefined
            ? memberType.typeName.getText()
            : undefined;
        return {
          questionOrDefault: Boolean(member?.questionToken) || isOptionalUnion(memberType),
          typeName,
        };
      }
      // A destructured parameter with no inline `{ ... }` type literal (e.g. a named type
      // reference) — this generator does not resolve a member's own optionality through it;
      // the documented heuristic below defaults such a key to required.
      return { questionOrDefault: false, typeName: undefined };
    }
  }
  return undefined;
}

/** The nearest enclosing function-like node — the property's own value is one in every real
 *  call site this repo has (an arrow function), but the walk is generic rather than assuming it. */
function enclosingFunction(node: ts.Node): ts.FunctionLikeDeclarationBase | undefined {
  let cur: ts.Node | undefined = node;
  while (cur) {
    if (ts.isArrowFunction(cur) || ts.isFunctionExpression(cur) || ts.isFunctionDeclaration(cur)) {
      return cur;
    }
    cur = cur.parent;
  }
  return undefined;
}

type Uncatalogued = { command: string; reason: string };

/** Parse one `invoke(...)` call's arguments into catalogue args, or record it as uncatalogued.
 *  `null` return means "recorded as uncatalogued; nothing to add to the entry map". */
function parseInvokeCall(
  call: ts.CallExpression,
  sf: ts.SourceFile,
  sources: FieldSources,
  uncatalogued: Uncatalogued[]
): { command: string; args: CatalogueArg[] } | null {
  const cmdArg = call.arguments[0];
  if (!cmdArg || !ts.isStringLiteral(cmdArg)) {
    // No command name to even file this under — nothing informative to record.
    return null;
  }
  const command = cmdArg.text;
  const argsArg = call.arguments[1];
  if (!argsArg) return { command, args: [] };

  if (!ts.isObjectLiteralExpression(argsArg)) {
    uncatalogued.push({ command, reason: 'second invoke() argument is not an object literal' });
    return null;
  }

  const fn = enclosingFunction(call);
  const args: CatalogueArg[] = [];
  for (const prop of argsArg.properties) {
    if (ts.isSpreadAssignment(prop)) {
      uncatalogued.push({ command, reason: 'spread in the invoke() args object' });
      return null;
    }
    if (!prop.name || ts.isComputedPropertyName(prop.name)) {
      uncatalogued.push({ command, reason: 'computed key in the invoke() args object' });
      return null;
    }
    const key = prop.name.getText(sf);

    let valueName: string | undefined;
    if (ts.isShorthandPropertyAssignment(prop)) {
      valueName = prop.name.text;
    } else if (ts.isPropertyAssignment(prop) && ts.isIdentifier(prop.initializer)) {
      valueName = prop.initializer.text;
    }
    // A literal/expression value (e.g. `boardId: 'linkedin'`) has no parameter to look up —
    // it is always sent, so the documented heuristic's default (required) is exactly right.
    const binding = valueName && fn ? findParamBinding(fn, valueName) : undefined;
    const required = !binding?.questionOrDefault;
    // A1-r1-AC-3 MEDIUM: `UNRESOLVED_WRAPPER_TYPE` (a syntactic shape `findParamBinding` already
    // positively identified as an object wrapper — rest-destructure, `unknown`, an inline type
    // literal, an indexed-access type AT THE PARAM SITE) always falls back to `null` ("known
    // wrapper, unresolved"). A plain NAMED type reference that fails `resolveNestedFields` is
    // ambiguous — it could be a genuine unresolved object (`ResumePipelineRunRequest`, an
    // `Omit<...> & ...` intersection; `PerformanceBackendConfig`, an interface declared outside
    // `ipc/contracts/*.ts`) or a scalar declared elsewhere (`PipelineStage`, a string-union alias
    // in `events/pipeline.ts`) — collapsing both into `null` used to publish the scalar case as an
    // object wrapper on the wire. `scalarTypeAliases` (a narrow, PROVEN-scalar lookup — see its own
    // doc) disambiguates the second case only; anything not proven scalar keeps the conservative
    // `null` default.
    const fields =
      binding?.typeName === UNRESOLVED_WRAPPER_TYPE
        ? null
        : binding?.typeName
          ? (resolveNestedFields(sources, binding.typeName) ??
            (sources.scalarTypeAliases.has(binding.typeName) ? undefined : null))
          : undefined;
    args.push({ name: key, required, fields });
  }
  return { command, args };
}

/** Every `invoke(...)` call anywhere in `node`'s subtree — regardless of type-argument shape, so
 *  `invoke<Foo>('x', ...)` and a multi-line generic are both found (the reason this walks the real
 *  AST rather than a regex). */
function findInvokeCalls(node: ts.Node): ts.CallExpression[] {
  const found: ts.CallExpression[] = [];
  const visit = (n: ts.Node) => {
    if (ts.isCallExpression(n) && ts.isIdentifier(n.expression) && n.expression.text === 'invoke') {
      found.push(n);
    }
    ts.forEachChild(n, visit);
  };
  visit(node);
  return found;
}

/** Canonical string for a [`CatalogueArg.fields`] value that keeps `undefined` (scalar) and `null`
 *  (unresolved wrapper) distinguishable — see that field's own doc. Used only for the two-call-site
 *  equality check below, never rendered. */
function fieldsKey(fields: string[] | null | undefined): string {
  if (fields === undefined) return 'scalar';
  if (fields === null) return 'unresolved-wrapper';
  return JSON.stringify(fields);
}

/** `true` when two `invoke()` call sites for the SAME command declared the identical arg
 *  contract — name, required-ness, and nested-field shape, order-independent (a param object's
 *  property order is not semantically load-bearing). */
function argsEqual(a: CatalogueArg[], b: CatalogueArg[]): boolean {
  if (a.length !== b.length) return false;
  const byName = new Map(a.map((arg) => [arg.name, arg]));
  return b.every((arg) => {
    const other = byName.get(arg.name);
    return (
      !!other &&
      other.required === arg.required &&
      fieldsKey(other.fields) === fieldsKey(arg.fields)
    );
  });
}

function processNamespaceFile(
  file: string,
  descCtx: DescCtx,
  fieldSources: FieldSources,
  entries: Map<string, CatalogueEntry>,
  uncatalogued: Uncatalogued[]
) {
  const namespace = file.split('/').slice(-2, -1)[0] ?? '';
  const text = readFileSync(abs(file), 'utf8');
  const sf = ts.createSourceFile(file, text, ts.ScriptTarget.Latest, true);

  const mainConst = sf.statements.find(
    (s): s is ts.VariableStatement =>
      ts.isVariableStatement(s) &&
      isExported(s) &&
      s.declarationList.declarations.some(
        (d) => d.initializer && ts.isObjectLiteralExpression(d.initializer)
      )
  );
  if (!mainConst) return;
  const decl = mainConst.declarationList.declarations.find(
    (d) => d.initializer && ts.isObjectLiteralExpression(d.initializer)
  );
  const obj = decl?.initializer as ts.ObjectLiteralExpression | undefined;
  if (!obj) return;

  for (const method of obj.properties) {
    if (!method.name || ts.isComputedPropertyName(method.name)) continue;
    const methodName = method.name.getText(sf);
    const calls = findInvokeCalls(method);
    for (const call of calls) {
      const parsed = parseInvokeCall(call, sf, fieldSources, uncatalogued);
      if (!parsed) continue;
      const description = describe(descCtx, namespace, methodName);
      const existing = entries.get(parsed.command);
      if (existing) {
        // A command invoked from more than one namespace (e.g. `boards.disconnect` AND
        // `linkedin.disconnect` both call `boards_logout`) — first-call-site-wins used to decide
        // the winner off `readdirSync` iteration ORDER (SECURITY/MEDIUM, CLI review round 1: two
        // genuinely different TSDocs meant the published description was an incidental
        // filesystem detail, not a deliberate choice). Silent when the two call sites AGREE
        // (the common, harmless case — same command reached two ways with identical docs);
        // `fail()`s only when they disagree, forcing a deliberate pick. Args are compared too
        // (A1-r1-AC-5/SEC-3 MEDIUM): the description twin of this hazard was already guarded, but
        // `parsed.args` — the shape actually ENFORCED at dispatch, via `check_input` — was silently
        // kept from whichever call site iteration reached first, so a future divergent second call
        // site would publish and enforce a contract derived from an arbitrary read-order pick.
        if (existing.description !== description || !argsEqual(existing.args, parsed.args)) {
          fail(
            `command "${parsed.command}" is invoked from more than one namespace with DIFFERING ` +
              `TSDoc descriptions or argument shapes ("${existing.description}" vs "${description}") ` +
              `— the published catalogue contract would depend on directory read order. Make the ` +
              `two call sites' TSDoc comments and argument shapes agree, or route the second call ` +
              `site through the first's own contract member.`
          );
        }
        continue;
      }
      entries.set(parsed.command, { command: parsed.command, description, args: parsed.args });
    }
  }
}

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
 *  MEASURE how many lines an entry costs while packing shards (`shardEntries`), so the two can
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

/** Greedily pack `entries` (already sorted) into shards, each capped at [`SHARD_LINE_BUDGET`]
 *  rendered lines — never a fixed shard COUNT, which would need bumping by hand as the catalogue
 *  grows, and never a fixed entries-per-shard count, which a few arg-heavy commands could blow
 *  past the LOC cap despite looking "even" by entry count. This is pure DATA (a `CatalogueEntry`/
 *  `CatalogueArg` struct literal), not logic that could be reorganized to fit this crate's own R8
 *  hard LOC cap (`docs/architecture-rules.md`) another way: rustfmt's own default `struct_lit_width`
 *  (18, far below any entry rendered here) forces one field per line regardless of how short the
 *  whole literal is, so a single ~160-command file cannot fit under the cap at all. Mirrors
 *  `ipc_contracts`' own per-domain file split (`gen-ipc-rust.ts`'s `MODULES`), sized by LINE BUDGET
 *  instead of by domain since this table has no natural per-domain boundary of its own. */
function shardEntries(entries: CatalogueEntry[]): CatalogueEntry[][] {
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

function shardFileName(shardNumber: number): string {
  return `shard_${shardNumber}.rs`;
}

function renderShardFile(
  shardNumber: number,
  shardCount: number,
  entries: CatalogueEntry[]
): string {
  const lines: string[] = [
    '// @generated by `pnpm gen:agent-catalogue` — DO NOT EDIT BY HAND.',
    `// Shard ${shardNumber} of ${shardCount} of the sharded command catalogue — see`,
    "// `../catalogue.rs`'s own doc for why this table is split at all.",
    '',
    'use super::{CatalogueArg, CatalogueEntry};',
    '',
    'pub(super) const ENTRIES: &[CatalogueEntry] = &[',
  ];
  for (const entry of entries) lines.push(...renderEntryLines(entry));
  lines.push('];');
  lines.push('');
  return lines.join('\n');
}

function renderAggregator(
  shardCount: number,
  totalEntries: number,
  uncataloguedNames: string[]
): string {
  const lines: string[] = [
    '// @generated by `pnpm gen:agent-catalogue` — DO NOT EDIT BY HAND.',
    '// Source of truth: apps/desktop/src/tauri-client/namespaces/**/*.ts (argument shape)',
    '//   + packages/shared/src/ipc/contracts/*.ts (description).',
    '// Generator: packages/shared/scripts/gen-agent-catalogue.ts. Run `pnpm gen:agent-catalogue`.',
    '// CI runs `pnpm gen:agent-catalogue:check` to catch drift.',
    '//',
    `// ${totalEntries} commands catalogued across ${shardCount} shard file(s) (catalogue/shard_*.rs),`,
    `// ${uncataloguedNames.length} uncatalogued (see UNCATALOGUED below).`,
    '//',
    "// Sharded rather than one big const array — this crate's own R8 hard LOC cap",
    "// (docs/architecture-rules.md) has no exception for generated DATA, and rustfmt's own default",
    '// `struct_lit_width` forces one field per line regardless of how short the whole literal is,',
    "// so this file's size scales with the command count with no upper bound this generator",
    '// controls. A `LazyLock<Vec<_>>` — never a `const` array-concat, which Rust cannot express',
    '// across separately-compiled const items without an allocation — is transparent to every call',
    '// site: `Deref<Target = Vec<CatalogueEntry>>` -> `Deref<Target = [CatalogueEntry]>` means',
    '// `CATALOGUE.iter()`/`.find(...)` read exactly as they would against a plain slice.',
    '',
    'use std::sync::LazyLock;',
    '',
  ];
  for (let n = 1; n <= shardCount; n++) lines.push(`mod shard_${n};`);
  lines.push('');
  lines.push(
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
    ''
  );
  lines.push(
    "/// One dispatchable command's declared input contract (issues #1163, #1158, #1160).",
    '#[derive(Clone, Copy)]',
    'pub(crate) struct CatalogueEntry {',
    "    pub(crate) command: &'static str,",
    "    pub(crate) description: &'static str,",
    "    pub(crate) args: &'static [CatalogueArg],",
    '}',
    ''
  );
  lines.push(
    "/// The full catalogue, assembled from every shard's own `ENTRIES` at first access — see this",
    "/// file's own header comment for why a `LazyLock<Vec<_>>` and not a plain `const` slice.",
    'pub(crate) static CATALOGUE: LazyLock<Vec<CatalogueEntry>> = LazyLock::new(|| {',
    `    let mut entries = Vec::with_capacity(${totalEntries});`
  );
  for (let n = 1; n <= shardCount; n++) {
    lines.push(`    entries.extend_from_slice(shard_${n}::ENTRIES);`);
  }
  lines.push('    entries', '});', '');
  lines.push(
    '/// Commands with at least one `invoke()` call this generator could not parse with confidence'
  );
  lines.push(
    '/// (a computed key, a spread, a non-literal command name, or a non-object second argument) —'
  );
  lines.push(
    '/// listed rather than silently dropped. A command with NO `invoke()` call at all (zero'
  );
  lines.push(
    "/// renderer references — see `policy.rs`'s own module doc) is absent from here too; the"
  );
  lines.push(
    '/// coverage test pairs both arrays with a hand-written allowlist for exactly that case.'
  );
  lines.push(
    '// Read only from `#[cfg(test)]` code today (the coverage test above) — a plain, non-test',
    '// `cargo check --lib` sees no reader at all, so this stays legitimately unused OUTSIDE tests',
    "// (never a silenced real finding — the RUST equivalent of `policy.rs`'s own module-level",
    '// allow).'
  );
  lines.push('#[allow(dead_code)]');
  lines.push('pub(crate) const UNCATALOGUED: &[&str] = &[');
  for (const name of uncataloguedNames) lines.push(`    ${rustStr(name)},`);
  lines.push('];');
  lines.push('');
  return lines.join('\n');
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

// ── Main ──────────────────────────────────────────────────────────────────────────────────────

async function main() {
  const descCtx = collectContractDescriptions();

  const schemasSf = ts.createSourceFile(
    SCHEMAS_INDEX,
    readFileSync(abs(SCHEMAS_INDEX), 'utf8'),
    ts.ScriptTarget.Latest,
    true
  );
  const zodAliases = collectZodTypeAliases(schemasSf);
  const zodSchemas = (await import('../src/schemas/index.js')) as unknown as Record<
    string,
    unknown
  >;

  const contractSources = parseContractFiles();
  const interfaceFields = collectContractInterfaceFields(contractSources);
  const scalarTypeAliases = collectScalarTypeAliasNames();
  const fieldSources: FieldSources = { zodAliases, zodSchemas, interfaceFields, scalarTypeAliases };

  const entries = new Map<string, CatalogueEntry>();
  const uncatalogued: Uncatalogued[] = [];

  // `.sort()` both listings — `readdirSync` order is filesystem-dependent (POSIX scandir order on
  // Linux CI, NTFS index order locally), and this loop's first-call-site-wins duplicate handling
  // (`processNamespaceFile`) means an unsorted walk would let THAT incidental order decide which
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
      formatWithRustfmt(renderAggregator(shards.length, sortedEntries.length, uncataloguedNames)),
    ],
  ];
  shards.forEach((shard, i) => {
    const shardNumber = i + 1;
    outputs.push([
      join(SHARD_DIR, shardFileName(shardNumber)),
      formatWithRustfmt(renderShardFile(shardNumber, shards.length, shard)),
    ]);
  });

  // Shard files left over from a run that produced MORE shards than this one (the catalogue
  // shrank) — deleted rather than left as stale, since a `mod shard_N;` that no longer exists in
  // the aggregator would otherwise leave an orphaned, unreferenced file behind forever.
  const shardDirAbs = abs(SHARD_DIR);
  const currentShardFileNames = new Set(outputs.map(([p]) => basename(p)));
  const staleShardFiles = existsSync(shardDirAbs)
    ? readdirSync(shardDirAbs).filter(
        (f) => f.startsWith('shard_') && f.endsWith('.rs') && !currentShardFileNames.has(f)
      )
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

try {
  await main();
} catch (error) {
  const message = error instanceof Error ? error.message : String(error);
  const roots = [REPO_ROOT, REPO_ROOT.split('\\').join('/')];
  console.error(roots.reduce((text, root) => text.split(root).join('.'), message));
  process.exitCode = 1;
}
