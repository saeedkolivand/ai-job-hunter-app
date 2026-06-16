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
      .replace(/\*([^*]+)\*/g, '$1') // single italic → plain
      .replace(/`(.+?)`/g, '$1')
      .replace(/```[\s\S]*?```/g, '')
      .trim()
  );
}
