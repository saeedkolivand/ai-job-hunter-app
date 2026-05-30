/** Keyword-emphasis instruction block. */

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
