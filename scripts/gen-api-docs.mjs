// Generates docs/API.md from the IPC contracts themselves, so the reference can
// never drift from the code:
//
//   packages/shared/src/ipc/contracts/*.ts  →  docs/API.md
//
// The hand-written predecessor documented three namespaces that did not exist
// (`agent`, `search`, `shortcuts`) and omitted nine that did, while its header
// claimed to be current. Nothing here is hand-maintained: every namespace,
// method, signature, channel and type is read out of the contract source, and
// the prose is the TSDoc that sits on the contract members. To change the
// document, change the contracts (or the PREAMBLE below) and re-run.
//
// Parsing is done with the TypeScript compiler API — never regex. Signatures
// and type declarations are copied VERBATIM from the source text, so a
// signature can never be rendered plausibly-but-wrongly. Any construct this
// script does not know how to render is a hard error, never a guess.
//
// Run with `pnpm gen:api`; CI enforces freshness via `pnpm gen:api:check`
// (regenerate + `git diff --exit-code`).

import { readdirSync, readFileSync, statSync, writeFileSync } from 'node:fs';
import { dirname, join, posix, relative, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

import prettier from 'prettier';
import ts from 'typescript';

// Anchored to this file, never to `process.cwd()`: run from anywhere but the
// repo root, a cwd-relative generator reads no contracts and writes its output
// outside the repo.
const REPO_ROOT = resolve(dirname(fileURLToPath(import.meta.url)), '..');

// Repo-relative by design: these strings are printed into the page too.
const CONTRACT_DIR = 'packages/shared/src/ipc/contracts';
const INDEX_FILE = join(CONTRACT_DIR, 'index.ts');
const OUT_FILE = 'docs/API.md';

/** Absolute path for a repo-relative one — filesystem access only. */
function abs(p) {
  return resolve(REPO_ROOT, p);
}

/** Repo-relative, forward-slashed path (path privacy: never absolute). */
function repoPath(p) {
  return relative(REPO_ROOT, abs(p)).split('\\').join('/');
}

function fail(message) {
  throw new Error(`gen:api — ${message}`);
}

// ── Parsing ───────────────────────────────────────────────────────────────

/** @returns {Map<string, ts.SourceFile>} keyed by repo-relative file path. */
function parseContractFiles() {
  const dir = abs(CONTRACT_DIR);
  const files = readdirSync(dir)
    .filter((f) => f.endsWith('.ts'))
    .map((f) => join(dir, f));
  const parsed = new Map();
  for (const file of files) {
    const text = readFileSync(file, 'utf8');
    parsed.set(
      repoPath(file),
      ts.createSourceFile(file, text, ts.ScriptTarget.Latest, /* setParentNodes */ true)
    );
  }
  return parsed;
}

/**
 * The TSDoc block attached to `node`, as markdown.
 *
 * Read from the raw leading comment ranges rather than the internal `jsDoc`
 * property, so the author's own markdown (lists, bold, fenced spans) survives
 * byte-for-byte instead of being re-serialized.
 */
function docOf(node, sf) {
  const ranges = ts.getLeadingCommentRanges(sf.text, node.getFullStart()) ?? [];
  const block = ranges.filter((r) => sf.text.slice(r.pos, r.pos + 3) === '/**').pop();
  if (!block) return '';
  const raw = sf.text.slice(block.pos, block.end);
  const body = raw
    .replace(/^\/\*\*/, '')
    .replace(/\*\/$/, '')
    .split(/\r?\n/)
    .map((line) => line.replace(/^\s*\*/, ''));

  // Dedent by the smallest indent across non-blank lines, so the repo's
  // `*  continuation` style doesn't turn prose into an indented code block
  // while nested list indentation is preserved.
  const indents = body.filter((l) => l.trim() !== '').map((l) => l.match(/^ */)[0].length);
  const dedent = indents.length ? Math.min(...indents) : 0;
  return body
    .map((l) => l.slice(dedent))
    .join('\n')
    .replace(/\{@link\s+([^}|\s]+)(?:[|\s][^}]*)?\}/g, '`$1`')
    .trim();
}

/** Source text of `node`, verbatim, excluding its TSDoc block. */
function signatureText(node, sf) {
  const text = sf.text.slice(node.getStart(sf, /* includeJsDocComment */ false), node.getEnd());
  if (text.startsWith('/*')) {
    fail(`doc comment leaked into a signature in ${repoPath(sf.fileName)}`);
  }
  return text;
}

/** Source text of `node` including its TSDoc block. */
function declarationText(node, sf) {
  return sf.text.slice(node.getStart(sf, /* includeJsDocComment */ true), node.getEnd());
}

function isExported(node) {
  return (node.modifiers ?? []).some((m) => m.kind === ts.SyntaxKind.ExportKeyword);
}

/**
 * Record `name -> entry`, failing on a collision.
 *
 * These maps are keyed by bare declaration name across every contract file, so
 * a name exported by two files would silently overwrite the first: a namespace
 * would then resolve to the wrong file's declaration and this page would render
 * another file's interface under it — plausibly, and wrongly. Qualifying the key
 * by file would paper over a real clash instead of surfacing it, so a duplicate
 * export is a hard error for a human to resolve.
 */
function addUnique(map, name, entry, kind) {
  const prior = map.get(name);
  if (prior) {
    fail(
      `duplicate exported ${kind} \`${name}\` in ${prior.file} and ${entry.file} — ` +
        'contract exports share a single name space here; rename one'
    );
  }
  map.set(name, entry);
}

/** Every exported interface / type alias in the contract files, by name. */
function collectDeclarations(sources) {
  const decls = new Map();
  for (const [file, sf] of sources) {
    for (const stmt of sf.statements) {
      if (
        (ts.isInterfaceDeclaration(stmt) || ts.isTypeAliasDeclaration(stmt)) &&
        isExported(stmt)
      ) {
        addUnique(decls, stmt.name.text, { file, sf, node: stmt }, 'type');
      }
    }
  }
  return decls;
}

/** Every exported `const X = {...} as const` in the contract files, by name. */
function collectConsts(sources) {
  const consts = new Map();
  for (const [file, sf] of sources) {
    for (const stmt of sf.statements) {
      if (!ts.isVariableStatement(stmt) || !isExported(stmt)) continue;
      for (const decl of stmt.declarationList.declarations) {
        if (ts.isIdentifier(decl.name)) {
          addUnique(consts, decl.name.text, { file, sf, node: decl }, 'const');
        }
      }
    }
  }
  return consts;
}

/** `namespace -> contract interface name`, read from `IpcContract` in index.ts. */
function namespaceMap(sf) {
  const iface = sf.statements.find(
    (s) => ts.isInterfaceDeclaration(s) && s.name.text === 'IpcContract'
  );
  if (!iface) fail(`interface IpcContract not found in ${repoPath(INDEX_FILE)}`);
  const map = new Map();
  for (const member of iface.members) {
    if (!ts.isPropertySignature(member) || !member.type) {
      fail(`unsupported IpcContract member: ${member.getText(sf)}`);
    }
    if (!ts.isTypeReferenceNode(member.type) || !ts.isIdentifier(member.type.typeName)) {
      fail(`IpcContract.${member.name.getText(sf)} is not a plain contract reference`);
    }
    map.set(member.name.getText(sf), member.type.typeName.text);
  }
  return map;
}

/** `namespace -> channels const name`, read from `IPC_CHANNELS` in index.ts. */
function channelConstMap(sf) {
  const map = new Map();
  for (const stmt of sf.statements) {
    if (!ts.isVariableStatement(stmt)) continue;
    for (const decl of stmt.declarationList.declarations) {
      if (!ts.isIdentifier(decl.name) || decl.name.text !== 'IPC_CHANNELS') continue;
      const init = ts.isAsExpression(decl.initializer)
        ? decl.initializer.expression
        : decl.initializer;
      if (!init || !ts.isObjectLiteralExpression(init))
        fail('IPC_CHANNELS is not an object literal');
      for (const prop of init.properties) {
        if (!ts.isPropertyAssignment(prop) || !ts.isIdentifier(prop.initializer)) {
          fail(`unsupported IPC_CHANNELS entry: ${prop.getText(sf)}`);
        }
        map.set(prop.name.getText(sf), prop.initializer.text);
      }
    }
  }
  if (map.size === 0) fail(`IPC_CHANNELS not found in ${repoPath(INDEX_FILE)}`);
  return map;
}

/** `method -> channel string` for one `*_CHANNELS` const. */
function readChannels(entry) {
  const { sf, node } = entry;
  const init =
    node.initializer && ts.isAsExpression(node.initializer)
      ? node.initializer.expression
      : node.initializer;
  if (!init || !ts.isObjectLiteralExpression(init)) {
    fail(`${node.name.text} is not an object literal in ${repoPath(sf.fileName)}`);
  }
  const channels = [];
  for (const prop of init.properties) {
    if (!ts.isPropertyAssignment(prop) || !ts.isStringLiteral(prop.initializer)) {
      fail(`${node.name.text} has a non-literal entry: ${prop.getText(sf)}`);
    }
    channels.push([prop.name.getText(sf).replace(/^'|'$/g, ''), prop.initializer.text]);
  }
  return channels;
}

/**
 * The file a relative import specifier points at, repo-relative.
 *
 * Contract sources spell a `.ts` file with the ESM `.js` specifier, and at
 * least one omits the extension altogether — rewriting the specifier's spelling
 * published `…/contracts/boards`, a path that does not exist, as a link.
 * Resolve against the files on disk instead, and fail rather than emit a link
 * that resolves to nothing.
 */
function resolveSpecifier(sf, spec) {
  const base = resolve(dirname(sf.fileName), spec).replace(/\.js$/, '.ts');
  const target = [base, `${base}.ts`, join(base, 'index.ts')].find((candidate) =>
    statSync(candidate, { throwIfNoEntry: false })?.isFile()
  );
  if (!target) {
    fail(`${repoPath(sf.fileName)} imports '${spec}', which resolves to no file`);
  }
  return repoPath(target);
}

/** Type names imported into a contract file, grouped by their source module. */
function importedTypes(sf) {
  const groups = new Map();
  for (const stmt of sf.statements) {
    if (!ts.isImportDeclaration(stmt) || !stmt.importClause) continue;
    const spec = stmt.moduleSpecifier.text;
    const where = `${repoPath(sf.fileName)} imports '${spec}'`;
    // Anything but a named import is skipped silently by a plain `continue`,
    // and its types then go undocumented with no sign anything was dropped.
    if (stmt.importClause.name) fail(`${where} as a default import — use a named import`);
    const bindings = stmt.importClause.namedBindings;
    if (bindings && ts.isNamespaceImport(bindings)) {
      fail(`${where} as a namespace import — name the types instead`);
    }
    if (!bindings) continue;
    const target = spec.startsWith('.') ? resolveSpecifier(sf, spec) : spec;
    const names = bindings.elements.map((e) => e.name.text);
    groups.set(target, [...(groups.get(target) ?? []), ...names]);
  }
  return groups;
}

/** The renderer-side protocol version constant, read from index.ts. */
function protocolVersion(sf) {
  for (const stmt of sf.statements) {
    if (!ts.isVariableStatement(stmt)) continue;
    for (const decl of stmt.declarationList.declarations) {
      if (ts.isIdentifier(decl.name) && decl.name.text === 'PROTOCOL_VERSION') {
        if (!decl.initializer || !ts.isStringLiteral(decl.initializer)) {
          fail('PROTOCOL_VERSION is not a string literal');
        }
        return decl.initializer.text;
      }
    }
  }
  return fail(`PROTOCOL_VERSION not found in ${repoPath(INDEX_FILE)}`);
}

// ── Rendering ─────────────────────────────────────────────────────────────

/** GitHub's heading-anchor slug (backticks and dots are dropped). */
function anchor(heading) {
  return heading
    .toLowerCase()
    .replace(/[^\w\- ]/g, '')
    .trim()
    .replace(/ /g, '-');
}

/** First sentence of a doc block, collapsed onto one line for a table cell. */
function summarize(doc) {
  if (!doc) return '';
  const firstPara = doc
    .split(/\n\s*\n/)[0]
    .replace(/\s*\n\s*/g, ' ')
    .trim();
  const sentence = /^(.*?[.:])(\s|$)/.exec(firstPara);
  return (
    (sentence ? sentence[1] : firstPara)
      // Backslash first: escaping the pipe first would leave its own escape
      // escapable, so a source `\` in front of a `|` would re-open the cell break.
      .replace(/\\/g, '\\\\')
      .replace(/\|/g, '\\|')
      // Whatever whitespace survived the paragraph collapse (a lone CR, a
      // line separator, a bullet-list newline) would otherwise end the row.
      .replace(/\s+/g, ' ')
      .trim()
  );
}

function fence(lang, body) {
  return ['```' + lang, body, '```'].join('\n');
}

function renderMember(namespace, member, sf) {
  if (!ts.isMethodSignature(member) && !ts.isPropertySignature(member)) {
    fail(
      `${namespace}: unsupported contract member kind ${ts.SyntaxKind[member.kind]} ` +
        `in ${repoPath(sf.fileName)} — extend the generator rather than guessing at its shape`
    );
  }
  const name = member.name.getText(sf);
  const doc = docOf(member, sf);
  const out = [`#### \`${namespace}.${name}\``, '', fence('ts', signatureText(member, sf))];
  if (doc) out.push('', doc);
  return { name, documented: doc !== '', markdown: out.join('\n') };
}

function renderNamespace(namespace, ctx) {
  const { decls, consts, channelConsts } = ctx;
  const contractName = ctx.namespaces.get(namespace);
  const contract = decls.get(contractName);
  if (!contract) fail(`${namespace}: interface ${contractName} not found in ${CONTRACT_DIR}`);
  if (!ts.isInterfaceDeclaration(contract.node)) {
    fail(`${namespace}: ${contractName} is a type alias, not an interface`);
  }
  const { sf, file, node } = contract;

  const out = [`## \`${namespace}\``, ''];
  const doc = docOf(node, sf);
  if (doc) out.push(doc, '');
  out.push(`Contract: \`${contractName}\` in \`${file}\``, '');

  const members = node.members.map((m) => renderMember(namespace, m, sf));
  out.push(`### Methods — \`${namespace}\``, '');
  out.push(
    members
      .map((m) => `- [\`${namespace}.${m.name}\`](#${anchor(`${namespace}.${m.name}`)})`)
      .join('\n'),
    ''
  );
  out.push(members.map((m) => m.markdown).join('\n\n'), '');

  // Channels
  const constName = channelConsts.get(namespace);
  out.push(`### Channels — \`${namespace}\``, '');
  if (!constName) {
    out.push(
      `\`${namespace}\` has no \`*_CHANNELS\` constant and is absent from \`IPC_CHANNELS\`.`,
      ''
    );
  } else {
    const entry = consts.get(constName);
    if (!entry) fail(`${namespace}: ${constName} not found in ${CONTRACT_DIR}`);
    const channels = readChannels(entry);
    if (channels.length === 0) {
      out.push(
        `\`${constName}\` in \`${entry.file}\` is declared and empty — no channel name is ` +
          "registered here. Read the constant's own doc comment for where the names come from.",
        ''
      );
    } else {
      out.push(`\`${constName}\` in \`${entry.file}\`:`, '');
      out.push('| Key | Channel |', '| --- | --- |');
      out.push(...channels.map(([k, v]) => `| \`${k}\` | \`${v}\` |`));
      out.push('');
      // A partial registry is a real, checkable fact — state the arithmetic
      // rather than inferring what the unregistered methods do instead.
      if (channels.length !== members.length) {
        out.push(
          `\`${constName}\` registers ${channels.length} of this namespace's ` +
            `${members.length} methods; the rest have no entry in it.`,
          ''
        );
      }
    }
  }

  // Types declared alongside the contract
  const localTypes = sf.statements.filter(
    (s) =>
      (ts.isInterfaceDeclaration(s) || ts.isTypeAliasDeclaration(s)) &&
      isExported(s) &&
      s.name.text !== contractName
  );
  if (localTypes.length > 0) {
    out.push(`### Types — \`${namespace}\``, '');
    out.push(`Declared in \`${file}\`.`, '');
    out.push(fence('ts', localTypes.map((t) => declarationText(t, sf)).join('\n\n')), '');
  }

  const imports = importedTypes(sf);
  if (imports.size > 0) {
    out.push(`### Referenced types — \`${namespace}\``, '');
    out.push(
      [...imports]
        .sort(([a], [b]) => a.localeCompare(b))
        .map(
          ([mod, names]) =>
            `- \`${mod}\` — ${[...new Set(names)]
              .sort()
              .map((n) => `\`${n}\``)
              .join(', ')}`
        )
        .join('\n'),
      ''
    );
  }

  return {
    markdown: out.join('\n'),
    methodCount: members.length,
    documentedCount: members.filter((m) => m.documented).length,
    summary: summarize(doc),
  };
}

const PREAMBLE = (version) =>
  [
    '<!-- GENERATED FILE — DO NOT EDIT.',
    `     Source: ${CONTRACT_DIR}/*.ts · Generator: scripts/gen-api-docs.mjs`,
    '     Change the contracts (signatures live in the code, prose lives in TSDoc on the',
    '     contract members) then run `pnpm gen:api`. CI fails on a stale copy. -->',
    '',
    '# IPC API Reference',
    '',
    `Every renderer ↔ Rust call is a typed contract in \`${CONTRACT_DIR}\`, and this page is`,
    'generated from those contracts. A method that is documented here exists; one that is not,',
    'does not. Methods with no description have no TSDoc on the contract yet — add it next to',
    'the signature, not here.',
    '',
    'The renderer reaches the shell exclusively through `AppClient`',
    '(`createTauriInvokeClient()` in `apps/desktop/src/tauri-client/index.ts`), consumed via the',
    'React Query service hooks in `apps/desktop/src/renderer/services/`.',
    '',
    '> **Never call `window.__TAURI_INVOKE__` directly.** Use the service hooks.',
    '',
    '## Transport',
    '',
    '| Direction       | Mechanism                      | Description                |',
    '| --------------- | ------------------------------ | -------------------------- |',
    '| Renderer → Rust | `tauri.invoke(cmd, payload)`   | Request/response (promise) |',
    '| Rust → Renderer | `tauri.listen(event, handler)` | Push events (subscription) |',
    '',
    'A subscription method (`onX(handler)`) returns its own unsubscribe function.',
    '',
    `Renderer protocol version: \`${version}\` (\`PROTOCOL_VERSION\`, \`${repoPath(INDEX_FILE)}\`).`,
    'It must match `system.getProtocolVersion()` from the shell; a mismatch means a',
    'partially-updated install.',
    '',
    'Adding a capability is a five-step change — contract, Rust command, `tauri-client`,',
    'service hook, query key — see `AGENTS.md` rule 14.',
    '',
  ].join('\n');

function renderIndex(rows) {
  const out = ['## Namespaces', '', '| Namespace | Methods | Summary |', '| --- | --- | --- |'];
  for (const row of rows) {
    out.push(
      `| [\`${row.namespace}\`](#${anchor(row.namespace)}) | ${row.methodCount} | ${row.summary} |`
    );
  }
  return out.join('\n');
}

// ── Main ──────────────────────────────────────────────────────────────────

async function main() {
  const sources = parseContractFiles();
  const indexSf = sources.get(repoPath(INDEX_FILE));
  if (!indexSf) fail(`${repoPath(INDEX_FILE)} not found`);

  const ctx = {
    namespaces: namespaceMap(indexSf),
    channelConsts: channelConstMap(indexSf),
    decls: collectDeclarations(new Map([...sources].filter(([f]) => f !== repoPath(INDEX_FILE)))),
    consts: collectConsts(new Map([...sources].filter(([f]) => f !== repoPath(INDEX_FILE)))),
  };

  // Every contract file must be reachable from IpcContract: a namespace that
  // exists in the directory but not in the union is invisible to the renderer,
  // and silently missing from this page is exactly the failure being fixed.
  const documentedFiles = new Set(
    [...ctx.namespaces.values()].map((name) => ctx.decls.get(name)?.file)
  );
  const orphans = [...sources.keys()].filter(
    (f) => f !== repoPath(INDEX_FILE) && !documentedFiles.has(f)
  );
  if (orphans.length > 0) {
    fail(`contract files not referenced by IpcContract: ${orphans.join(', ')}`);
  }

  const namespaces = [...ctx.namespaces.keys()].sort((a, b) => a.localeCompare(b));
  const rendered = namespaces.map((namespace) => ({
    namespace,
    ...renderNamespace(namespace, ctx),
  }));

  const body = [
    PREAMBLE(protocolVersion(indexSf)),
    renderIndex(rendered),
    '',
    rendered.map((r) => r.markdown).join('\n---\n\n'),
  ].join('\n');

  const config = await prettier.resolveConfig(abs(OUT_FILE));
  const formatted = await prettier.format(body, {
    ...config,
    parser: 'markdown',
    filepath: abs(OUT_FILE),
  });
  writeFileSync(abs(OUT_FILE), formatted);

  const methods = rendered.reduce((n, r) => n + r.methodCount, 0);
  const documented = rendered.reduce((n, r) => n + r.documentedCount, 0);
  console.log(
    `Generated ${posix.normalize(OUT_FILE)} — ${rendered.length} namespaces, ${methods} methods, ` +
      `${documented} with TSDoc (${methods - documented} without).`
  );
}

try {
  await main();
} catch (error) {
  // Path privacy: print the message alone. Node's default handler prints a
  // stack trace full of absolute paths, and a filesystem error carries one in
  // its message, so the repo root is stripped out of both.
  const message = error instanceof Error ? error.message : String(error);
  const roots = [REPO_ROOT, REPO_ROOT.split('\\').join('/')];
  console.error(roots.reduce((text, root) => text.split(root).join('.'), message));
  process.exitCode = 1;
}
