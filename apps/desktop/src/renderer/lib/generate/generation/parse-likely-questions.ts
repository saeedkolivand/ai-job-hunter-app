/** One likely interview question the CANDIDATE will be asked — session-only
 *  practice set (never persisted to the aiGenerations aggregate). */
export interface LikelyQuestion {
  id: string;
  question: string;
  /** behavioral | roleSpecific | technical (open-typed; unknown -> roleSpecific). */
  type: string;
}

/** Question-type ids the UI groups by; an unknown value normalizes to `roleSpecific`. */
const TYPES = ['behavioral', 'roleSpecific', 'technical'] as const;

/** Normalize a free-text type tag to a known id (lenient; default `roleSpecific`). */
function normalizeType(raw: string): string {
  const v = raw
    .trim()
    .toLowerCase()
    .replace(/[\s_-]/g, '');
  if (v === 'role' || v === 'rolespecific') return 'roleSpecific';
  return TYPES.find((t) => t.toLowerCase() === v) ?? 'roleSpecific';
}

/**
 * Lenient parser for the delimited likely-questions output (a `Q:` / `TYPE:`
 * block per item). Tolerates numbering/bullet prefixes, a missing `TYPE:` line,
 * and stray markdown; skips any block with no question text. Mirrors
 * `parseInterviewQuestions` — provider-agnostic, never assumes valid JSON.
 */
export function parseLikelyQuestions(raw: string): LikelyQuestion[] {
  const out: LikelyQuestion[] = [];
  let cur: { question: string; type: string } | null = null;

  const flush = () => {
    if (cur && cur.question.trim()) {
      out.push({
        id: `lq-${out.length + 1}`,
        question: cur.question.trim(),
        type: normalizeType(cur.type),
      });
    }
    cur = null;
  };

  for (const line of raw.split(/\r?\n/)) {
    const q = line.match(/^\s*(?:[-*\d.)]+\s*)?Q:\s*(.+)$/i);
    if (q) {
      flush();
      cur = { question: q[1] ?? '', type: 'roleSpecific' };
      continue;
    }
    if (!cur) continue;
    const type = line.match(/^\s*TYPE:\s*(.+)$/i);
    if (type) {
      cur.type = type[1] ?? '';
    }
  }
  flush();
  return out;
}
