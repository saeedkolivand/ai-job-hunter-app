import { abs, isExported, ts } from '../../../../scripts/gen-api-docs.mjs';
import { type DescCtx, describe } from './describe.js';
import { type FieldSources, readSource, resolveNestedFields } from './field-sources.js';
import { mergeCatalogueEntry } from './merge.js';
import type { CatalogueArg, CatalogueEntry, Uncatalogued } from './model.js';

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

export function processNamespaceFile(
  file: string,
  descCtx: DescCtx,
  fieldSources: FieldSources,
  entries: Map<string, CatalogueEntry>,
  descByNamespaceCommand: Map<string, string>,
  uncatalogued: Uncatalogued[]
) {
  const namespace = file.split('/').slice(-2, -1)[0] ?? '';
  const sf = readSource(abs(file));

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
      mergeCatalogueEntry(
        entries,
        descByNamespaceCommand,
        namespace,
        parsed.command,
        description,
        parsed.args
      );
    }
  }
}
