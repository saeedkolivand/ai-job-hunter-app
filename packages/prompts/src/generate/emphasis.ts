/** Keyword-emphasis + résumé-grounding instruction blocks. */

/**
 * Whether the résumé body actually mentions `term`.
 *
 * Multi-word terms or terms containing punctuation (e.g. `Node.js`, `CI/CD`,
 * `REST API`) match by case-insensitive substring; single alphanumeric tokens
 * (e.g. `React`, `Go`, `AWS`) match on a word boundary so `Go` does not match
 * "category". Heuristic by design — the model still sees the full résumé and is
 * told never to fabricate — but it lets us tell the model which job-ad
 * requirements are genuinely backed by the résumé.
 */
export function resumeMentions(resumeBody: string, term: string): boolean {
  const t = term.trim().toLowerCase();
  if (!t) return false;
  if (/[^a-z0-9]/.test(t)) return resumeBody.toLowerCase().includes(t);
  const esc = t.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
  return new RegExp(`\\b${esc}\\b`, 'i').test(resumeBody);
}

/**
 * Build a grounding block that splits the job-ad's top requirements into those
 * the résumé actually supports and those it does not — so the model emphasizes
 * real strengths and never claims absent skills. Complements
 * {@link buildEmphasisBlock} (which only bolds keywords "when they appear
 * naturally"); this makes the present/absent split explicit and verifiable.
 * Returns '' when there are no requirements to classify.
 */
export function buildGroundingBlock(resumeBody: string, topRequirements: string[]): string {
  const reqs = topRequirements.slice(0, 12);
  if (!reqs.length) return '';

  const present: string[] = [];
  const absent: string[] = [];
  for (const req of reqs) {
    (resumeMentions(resumeBody, req) ? present : absent).push(req);
  }
  if (!present.length && !absent.length) return '';

  const lines = ['SKILL GROUNDING — verified against the résumé (do NOT contradict):'];
  if (present.length) {
    lines.push(`- PRESENT — safe to emphasize and weave in naturally: ${present.join(', ')}`);
  }
  if (absent.length) {
    lines.push(
      `- ABSENT — the résumé does NOT show these; NEVER claim, imply, or bold them as the candidate's: ${absent.join(', ')}`
    );
  }
  return lines.join('\n');
}

/**
 * Build the bold emphasis instruction block for prompts.
 * The AI uses **keyword** notation; the renderer converts to real bold.
 */
export function buildEmphasisBlock(keywords: string[]): string {
  if (!keywords.length) return '';
  const list = keywords
    .slice(0, 12)
    .map((k) => `**${k}**`)
    .join(', ');
  return `
KEYWORD EMPHASIS — CRITICAL:
Wrap the following job-ad keywords in **double asterisks** when they appear naturally in your output:
${list}

Emphasis rules:
- Bold ONLY when the keyword appears in a genuinely relevant technical or skill context
- Bold the FIRST occurrence per section — not every instance
- Maximum 2–3 bolded terms per bullet point
- NEVER bold: company names, dates, pronouns, generic verbs, or section headers
- Bolding should feel strategic and natural — not keyword-stuffed
- The **asterisks** will be converted to real bold typography in the exported document

Example:
  WEAK:  Built frontend applications with React and TypeScript
  GOOD:  Built scalable **React** and **TypeScript** frontend applications integrated with **REST APIs**`;
}
