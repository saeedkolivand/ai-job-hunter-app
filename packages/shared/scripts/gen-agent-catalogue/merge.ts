import { type CatalogueArg, type CatalogueEntry, fail } from './model.js';

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

/** Merge one parsed `invoke()` call site into the accumulated `entries` map — the ONE place a
 *  command reached from more than one namespace (e.g. `boards.disconnect` AND
 *  `linkedin.disconnect` both call `boards_logout`) gets reconciled. Exported for
 *  `gen-agent-catalogue.test.ts` (issue #1183 F2).
 *
 *  Two DIFFERENT checks, deliberately keyed differently:
 *
 *  - **Args** (`argsEqual`) are keyed on the bare `command` — the shape actually ENFORCED at
 *    dispatch (`check_input`) is a single contract regardless of which namespace's TSDoc
 *    described it, so two namespaces publishing genuinely different argument shapes for the same
 *    dispatched command IS a real bug (A1-r1-AC-5/SEC-3 MEDIUM) and still `fail()`s.
 *  - **Descriptions** are keyed on `(namespace, command)`, not the bare command (issue #1183 F2
 *    fix). `LinkedinContract` and `BoardsContract` legitimately want their OWN wording for the
 *    same underlying `boards_logout`/`boards_connect_status` etc. — a bare-command key forced
 *    `linkedin.ts`'s TSDoc to be genericized to `boards.ts`'s wording to satisfy this same check,
 *    degrading `docs/API.md`'s LinkedIn-specific documentation for no safety reason: the catalogue
 *    only ever ENFORCES one arg shape per command (above), never one description, so which
 *    description wins is cosmetic. Keying on `(namespace, command)` still catches the case this
 *    guard actually exists for — the SAME namespace declaring two conflicting descriptions for one
 *    command (a real authoring mistake, not a deliberate per-namespace wording choice) — while
 *    letting two DIFFERENT namespaces disagree freely. First-namespace-wins (by the sorted
 *    `readdirSync` walk) decides which description is PUBLISHED when they legitimately differ;
 *    that pick is arbitrary but harmless, since nothing downstream validates against it. */
export function mergeCatalogueEntry(
  entries: Map<string, CatalogueEntry>,
  descByNamespaceCommand: Map<string, string>,
  namespace: string,
  command: string,
  description: string,
  args: CatalogueArg[]
): void {
  const nsCommandKey = `${namespace}\0${command}`;
  const priorNsDescription = descByNamespaceCommand.get(nsCommandKey);
  if (priorNsDescription !== undefined && priorNsDescription !== description) {
    fail(
      `command "${command}" is invoked more than once from the "${namespace}" namespace with ` +
        `DIFFERING TSDoc descriptions ("${priorNsDescription}" vs "${description}") — make the ` +
        `two call sites' TSDoc comments agree.`
    );
  }
  descByNamespaceCommand.set(nsCommandKey, description);

  const existing = entries.get(command);
  if (existing) {
    if (!argsEqual(existing.args, args)) {
      fail(
        `command "${command}" is invoked from more than one namespace with DIFFERING argument ` +
          `shapes — the published catalogue contract would depend on directory read order. Make ` +
          `the two call sites' argument shapes agree, or route the second call site through the ` +
          `first's own contract member.`
      );
    }
    return;
  }
  entries.set(command, { command, description, args });
}
