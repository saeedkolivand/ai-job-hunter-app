/**
 * Reverse the Rust BE's fence wrapper for a value the extension is about to
 * RENDER to the user. The desktop fences every `agentQuery` response with
 * `<job_posting>…</job_posting>` (see `prompt_fence.rs`'s `fenced`), so a
 * resource value read back through that surface carries the literal wrapper
 * — a trust line that printed it verbatim would show the user
 * `<job_posting>…</job_posting>` markup instead of the title/company it
 * wraps. This is the mirror of `prompt_fence.rs::strip_fence_wrapper`, the
 * Rust primitive `commands/scrape.rs` uses before persisting a
 * caller-echoed value: the SAME shape rule, transliterated, so the two
 * sides cannot drift apart and the extension strips exactly what Rust
 * wrote (see the owning Rust fn's doc for the full rationale — only
 * removes a wrapper matching `fenced`'s EXACT shape `<tag>\n{body}\n</tag>`;
 * every other string passes through byte-for-byte unchanged, so this is a
 * no-op on the normal path).
 */
export function stripFenceWrapper(tag: string, s: string): string {
  const open = `<${tag}>\n`;
  const close = `\n</${tag}>`;
  // Mirrors Rust's `strip_prefix(open).and_then(strip_suffix(close))` EXACTLY,
  // including its order: the suffix is checked on the REMAINDER, never on the
  // original — so the empty-body overlap `<tag>\n</tag>` does NOT match (its
  // remainder `</tag>` is shorter than `\n</tag>`, a `strip_suffix` miss, and
  // the value passes through unchanged), exactly like the Rust behaviour a
  // naive both-ends check on the original string would corrupt.
  if (!s.startsWith(open)) return s;
  const rest = s.slice(open.length);
  return rest.endsWith(close) ? rest.slice(0, rest.length - close.length) : s;
}
