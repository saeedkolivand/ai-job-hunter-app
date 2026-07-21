/** Post-generation plain-text cleanup of model output. */

export function extractPlainText(raw: string): string {
  return (
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
      .replace(/^#{1,6}\s/gm, '')
      .replace(/\*\*\*(.+?)\*\*\*/g, '**$1**') // triple → double (preserve bold)
      // Single italic → plain, but ONLY a `*` that is not part of a `**` run.
      // Without the guards this pass ate bold too: the leftmost match inside
      // `**bold**` is the inner `*bold*`, leaving `*bold*` — and because
      // `[^*]+` also matches spaces and commas, two adjacent bold spans paired
      // up ACROSS each other, so `**Python**, **Go**` collapsed to
      // `*Python, Go*`. The prompts ask for 2-3 `**keyword**` bolds per bullet,
      // so this ran on essentially every generated document.
      .replace(/(?<!\*)\*(?!\*)([^*]+)\*(?!\*)/g, '$1')
      // Fenced blocks first — otherwise the inline-backtick pass below consumes
      // the ``` fence markers, orphaning them so the fenced regex no longer
      // matches and the code body leaks into the plain text.
      .replace(/```[\s\S]*?```/g, '')
      .replace(/`(.+?)`/g, '$1')
      .trim()
  );
}
