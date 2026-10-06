import {
  docOf,
  INDEX_FILE,
  isExported,
  namespaceMap,
  parseContractFiles,
  repoPath,
  ts,
} from '../../../../scripts/gen-api-docs.mjs';
import { fail } from './model.js';

// ── Contract descriptions (namespace + method -> first TSDoc sentence) ────────────────────────

export interface DescCtx {
  namespaces: Map<string, string>;
  decls: Map<string, { file: string; sf: ts.SourceFile; node: ts.Node }>;
}

export function collectContractDescriptions(): DescCtx {
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
export function describe(ctx: DescCtx, namespace: string, method: string): string {
  const contractName = ctx.namespaces.get(namespace);
  if (!contractName) return '';
  const contract = ctx.decls.get(contractName);
  if (!contract || !ts.isInterfaceDeclaration(contract.node)) return '';
  const member = contract.node.members.find((m) => m.name?.getText(contract.sf) === method);
  if (!member) return '';
  return catalogueSummarize(docOf(member, contract.sf));
}
