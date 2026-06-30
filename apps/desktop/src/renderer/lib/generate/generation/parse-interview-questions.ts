import type { InterviewQuestion } from '@ajh/shared';

/** Audience ids the UI groups by; an unknown value normalizes to `general`. */
const AUDIENCES = ['recruiter', 'hiringManager', 'team', 'leadership', 'general'] as const;

/** Normalize a free-text audience tag to a known id (lenient; default `general`). */
function normalizeAudience(raw: string): string {
  const v = raw
    .trim()
    .toLowerCase()
    .replace(/[\s_-]/g, '');
  if (v === 'hiringmanager' || v === 'manager' || v === 'hm') return 'hiringManager';
  return AUDIENCES.find((a) => a.toLowerCase() === v) ?? 'general';
}

/**
 * Lenient parser for the delimited interview-questions output (a `Q:` / `WHY:` /
 * `AUDIENCE:` block per item). Tolerates numbering/bullet prefixes, missing
 * `WHY`/`AUDIENCE` lines, and stray markdown; skips any block with no question
 * text. Provider-agnostic — never assumes valid JSON (zero-change provider rule).
 */
export function parseInterviewQuestions(raw: string): InterviewQuestion[] {
  const out: InterviewQuestion[] = [];
  let cur: { question: string; why: string; audience: string } | null = null;

  const flush = () => {
    if (cur && cur.question.trim()) {
      out.push({
        id: `iq-${out.length + 1}`,
        question: cur.question.trim(),
        why: cur.why.trim(),
        audience: normalizeAudience(cur.audience),
      });
    }
    cur = null;
  };

  for (const line of raw.split(/\r?\n/)) {
    const q = line.match(/^\s*(?:[-*\d.)]+\s*)?Q:\s*(.+)$/i);
    if (q) {
      flush();
      cur = { question: q[1] ?? '', why: '', audience: 'general' };
      continue;
    }
    if (!cur) continue;
    const why = line.match(/^\s*WHY:\s*(.+)$/i);
    if (why) {
      cur.why = why[1] ?? '';
      continue;
    }
    const audience = line.match(/^\s*AUDIENCE:\s*(.+)$/i);
    if (audience) {
      cur.audience = audience[1] ?? '';
    }
  }
  flush();
  return out;
}
