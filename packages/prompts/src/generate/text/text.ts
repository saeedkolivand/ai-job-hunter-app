/** Post-generation plain-text cleanup of model output. */

/**
 * A local model that wraps its ENTIRE answer in one code fence — the same
 * Ollama tell already fixed in `github-projects.ts`'s `parseGitHubProjects`
 * — must have that fence unwrapped, not deleted: every caller of
 * `extractPlainText` generates prose (résumé/cover-letter/application-answer/
 * etc.), never a real code artifact, so a fence spanning the WHOLE trimmed
 * response can only be spurious wrapping, and dropping it drops the whole
 * document. A fence embedded mid-answer (e.g. a genuine example snippet the
 * model quotes) is a different case — it isn't the entire response, so it
 * still falls through to the delete pass below, unchanged.
 */
function unwrapIfWholeFence(text: string): string {
  const trimmed = text.trim();
  const match = trimmed.match(/^```[a-zA-Z0-9_-]*\r?\n([\s\S]*?)\r?\n?```$/);
  if (!match) return text;
  const body = match[1] ?? '';
  // A second fence marker inside means this isn't a single whole-answer wrap
  // (e.g. two fenced blocks back to back) — leave it for the delete pass
  // below rather than guess which block is "the" answer.
  return body.includes('```') ? text : body;
}

export function extractPlainText(raw: string): string {
  const result = unwrapIfWholeFence(
    raw
      // Tempered, linear forms of the lazy-dotall strips: `[\s\S]*?</tag>` with a
      // multichar terminator backtracks polynomially (js/polynomial-redos). The
      // `(?:(?!</tag>)[\s\S])*` shape matches the same spans with no backtracking.
      .replace(/<think>(?:(?!<\/think>)[\s\S])*<\/think>/gi, '') // local model thinking blocks
      .replace(/<leakage_check>(?:(?!<\/leakage_check>)[\s\S])*<\/leakage_check>/gi, '') // legacy self-check block
      // Strip any XML wrapper tags the model might echo from the prompt
      .replace(/<\/?candidate_resume>/gi, '')
      .replace(/<\/?job_ad>/gi, '')
      .replace(/<\/?leakage_check>/gi, '') // stray unclosed tags
  )
    .replace(/^#{1,6}\s/gm, '')
    .replace(/\*\*\*(.+?)\*\*\*/g, '**$1**') // triple → double (preserve bold)
    // Single italic → plain, but ONLY a `*` that is not part of a `**` run.
    // Without the guards this pass ate bold too: the leftmost match inside
    // `**bold**` is the inner `*bold*`, leaving `*bold*` — and because
    // `[^*]+` also matches spaces and commas, two adjacent bold spans paired
    // up ACROSS each other, so `**Python**, **Go**` collapsed to
    // `*Python, Go*`. The prompts ask for 2-3 `**keyword**` bolds per bullet,
    // so this ran on essentially every generated document.
    // `[^*\n]` (not `[^*]`) keeps the class inside one line — otherwise a
    // `* apple\n* banana` markdown bullet list pairs the two leading list
    // stars across the newline, eating them and leaving ` apple\n banana`.
    .replace(/(?<!\*)\*(?!\*)([^*\n]+)\*(?!\*)/g, '$1')
    // Fenced blocks first — otherwise the inline-backtick pass below consumes
    // the ``` fence markers, orphaning them so the fenced regex no longer
    // matches and the code body leaks into the plain text.
    .replace(/```[\s\S]*?```/g, '')
    .replace(/`(.+?)`/g, '$1')
    .trim();

  // Diagnostic only — lengths, never content. A non-empty response that comes
  // out empty means one of the passes above (or a legitimately think-only
  // response, e.g. a reasoning model that never reached its final channel)
  // ate the whole thing; a support bundle otherwise has nothing to show for
  // why a generation came back blank (see the whole-fence unwrap above, added
  // for exactly this failure mode).
  if (raw.trim() && !result) {
    console.warn(`extractPlainText: emptied a non-empty response (rawLength=${raw.length})`);
  }

  return result;
}
