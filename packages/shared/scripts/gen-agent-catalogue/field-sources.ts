import { readdirSync, readFileSync, statSync } from 'node:fs';
import { join } from 'node:path';

import { z } from 'zod';

import { abs, isExported, ts } from '../../../../scripts/gen-api-docs.mjs';
import { fail, SCHEMAS_DIR } from './model.js';

/** Parse the TypeScript file at `path` — the one place a source file is read for analysis. */
export function readSource(path: string): ts.SourceFile {
  return ts.createSourceFile(path, readFileSync(path, 'utf8'), ts.ScriptTarget.Latest, true);
}

// ── Nested field names for a wrapper key's type ────────────────────────────────────────────────

/** `TypeName -> SchemaConstName`, read from every `export type X = z.infer<typeof Y>;` across
 *  `packages/shared/src/schemas/*.ts` — the one naming convention every generated Zod request
 *  type in this repo follows. */
export function collectZodTypeAliases(): Map<string, string> {
  const map = new Map<string, string>();
  for (const file of listTsFiles(abs(SCHEMAS_DIR))) {
    const sf = readSource(file);
    for (const stmt of sf.statements) {
      if (!ts.isTypeAliasDeclaration(stmt) || !isExported(stmt)) continue;
      const t = stmt.type;
      if (!ts.isTypeReferenceNode(t) || t.typeName.getText(sf) !== 'z.infer') continue;
      const arg = t.typeArguments?.[0];
      if (!arg || !ts.isTypeQueryNode(arg)) continue;
      map.set(stmt.name.text, arg.exprName.getText(sf));
    }
  }
  return map;
}

/** Order-independent equality of two field-name lists — a property's declaration order is not
 *  semantically load-bearing (same convention as `argsEqual`'s own `fieldsKey` comparison). */
function sameFieldSet(a: string[], b: string[]): boolean {
  if (a.length !== b.length) return false;
  const setA = new Set(a);
  return b.every((f) => setA.has(f));
}

/** Flat top-level member names of every exported interface declared directly in an
 *  `ipc/contracts/*.ts` file (never Zod-derived) — the second convention a wrapper key's type can
 *  follow, e.g. `BaseExportRequest`/`TemplateRecommendSignals`. Exported for
 *  `gen-agent-catalogue.test.ts` (the duplicate-name guard, A1-r2-AC-3 MEDIUM) — main() itself
 *  never runs on import, see the guard at the bottom of this file. */
export function collectContractInterfaceFields(
  sources: Map<string, ts.SourceFile>
): Map<string, string[]> {
  const map = new Map<string, string[]>();
  for (const [, sf] of sources) {
    for (const stmt of sf.statements) {
      if (!ts.isInterfaceDeclaration(stmt) || !isExported(stmt)) continue;
      const fields = stmt.members.filter(ts.isPropertySignature).map((m) => m.name.getText(sf));
      // Two exported interfaces sharing a name across different `ipc/contracts/*.ts` files (A1-r2-
      // AC-3 MEDIUM) — last-wins used to make the nested-key contract dispatch ENFORCES depend on
      // `readdirSync` order, the exact hazard the sibling multi-call-site guard already `fail()`s
      // on for descriptions/args. Silent when the two declarations AGREE (same fields, order-
      // independent); `fail()`s only on a genuine divergence.
      const existing = map.get(stmt.name.text);
      if (existing && !sameFieldSet(existing, fields)) {
        fail(
          `interface "${stmt.name.text}" is exported from more than one ipc/contracts/*.ts file ` +
            `with DIFFERING member lists — the nested-key contract dispatch enforces would depend ` +
            `on directory read order. Rename one of the two interfaces or make their members agree.`
        );
      }
      map.set(stmt.name.text, fields);
    }
  }
  return map;
}

export interface FieldSources {
  zodAliases: Map<string, string>;
  zodSchemas: Record<string, unknown>;
  interfaceFields: Map<string, string[]>;
  scalarTypeAliases: Set<string>;
}

/** Nested field names for a wrapper key typed as `typeName`, or `undefined` when this generator
 *  cannot resolve that type's shape (not an error — see the module doc: only a top-level construct
 *  it cannot parse at all is fatal). */
export function resolveNestedFields(sources: FieldSources, typeName: string): string[] | undefined {
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
export function collectScalarTypeAliasNames(): Set<string> {
  const names = new Set<string>();
  for (const file of listTsFiles(abs('packages/shared/src'))) {
    const sf = readSource(file);
    for (const stmt of sf.statements) {
      if (!ts.isTypeAliasDeclaration(stmt) || !isExported(stmt)) continue;
      if (isScalarTypeNode(sf, stmt.type)) names.add(stmt.name.text);
    }
  }
  return names;
}
