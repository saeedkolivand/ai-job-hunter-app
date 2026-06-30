/**
 * Streaming splitter for inline `<think>…</think>` reasoning that local models
 * (DeepSeek-R1, Qwen3, …) embed directly in their content stream.
 *
 * Cloud providers that flag reasoning structurally (`thinking: true` on the
 * stream chunk) are handled by the caller *before* `push` — this util only deals
 * with the embedded-tag case, so every streaming surface (AI Generate, analyze,
 * the autopilot apply modal) separates reasoning the same way instead of each
 * re-implementing the parser.
 *
 * Stateful across deltas: it holds back a few trailing characters so a `<think>`
 * tag split across two deltas is still detected. Visible answer text goes to
 * `onToken`; reasoning text goes to `onThinking`. Call `flush()` once at stream
 * end to emit any buffered trailing content (an unterminated think block is
 * discarded).
 */
export interface ThinkSplitter {
  /** Feed one non-thinking content delta from the stream. */
  push(delta: string): void;
  /** Emit trailing buffered visible content at stream end. */
  flush(): void;
}

const OPEN = '<think>';
const CLOSE = '</think>';

export function createThinkSplitter(
  onToken: (text: string) => void,
  onThinking: (text: string) => void
): ThinkSplitter {
  let inThinkBlock = false;
  let accum = '';

  const push = (delta: string): void => {
    accum += delta;

    let out = '';
    let remaining = accum;

    while (remaining.length > 0) {
      if (inThinkBlock) {
        const closeIdx = remaining.indexOf(CLOSE);
        if (closeIdx !== -1) {
          onThinking(remaining.slice(0, closeIdx));
          inThinkBlock = false;
          remaining = remaining.slice(closeIdx + CLOSE.length);
        } else {
          // Still inside the think block — forward all of it as reasoning.
          onThinking(remaining);
          remaining = '';
        }
      } else {
        const openIdx = remaining.indexOf(OPEN);
        if (openIdx !== -1) {
          out += remaining.slice(0, openIdx);
          inThinkBlock = true;
          remaining = remaining.slice(openIdx + OPEN.length);
        } else {
          // No open tag — but the tail might be a partial `<think>` arriving
          // split across deltas, so hold back its length and re-check next push.
          if (remaining.length > OPEN.length) {
            out += remaining.slice(0, remaining.length - OPEN.length);
            remaining = remaining.slice(remaining.length - OPEN.length);
          }
          break;
        }
      }
    }

    accum = remaining;
    if (out) onToken(out);
  };

  const flush = (): void => {
    // An unterminated think block is discarded; any trailing visible content is emitted.
    if (accum && !inThinkBlock) {
      onToken(accum);
    }
    accum = '';
  };

  return { push, flush };
}
