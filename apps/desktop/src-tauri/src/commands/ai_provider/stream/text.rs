//! Byte/text processing for the shared streaming loop: think-block stripping
//! and UTF-8-safe chunk buffering. Split out of `stream.rs` (R8 line-budget
//! split): pure text transforms, no provider/transport concerns.

/// Remove inline `<think>…</think>` reasoning blocks, mirroring the renderer's
/// `createThinkSplitter` (`renderer/lib/generate/think-split.ts`) so the
/// persisted answer text is byte-for-byte the shape the renderer assembles from
/// the live stream. Local reasoning models (DeepSeek-R1, Qwen3, …) embed the
/// tags directly in their answer content; cloud providers flag reasoning
/// structurally (those deltas never reach `answer` in the first place, since the
/// loop only accumulates non-thinking deltas), so for them this is a no-op.
///
/// Semantics match the splitter's final output exactly: text outside a block is
/// kept, text inside a `<think>…</think>` pair is dropped, and an UNTERMINATED
/// `<think>` (no closing tag) discards everything from that tag onward — the
/// splitter drops an unterminated block at `flush()`. Because the whole answer
/// is stripped in one pass here (not incrementally across deltas), a `</think>`
/// split across two stream frames — which the renderer's streaming splitter can
/// mis-handle — is resolved correctly, so the persisted text can only ever be
/// equal-or-more-correct than the buffer, and never contains reasoning markup.
///
/// Shared with the CLI-agent streaming path (`cli_agent::run_stream`), which has
/// its own subprocess transport but persists the completed answer the SAME way,
/// so a CLI-agent generation's poll fallback works too and no provider path can
/// leak `<think>` markup.
pub(in crate::commands::ai_provider) fn strip_think_blocks(text: &str) -> String {
    const OPEN: &str = "<think>";
    const CLOSE: &str = "</think>";
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    loop {
        match rest.find(OPEN) {
            Some(open) => {
                out.push_str(&rest[..open]);
                let after = &rest[open + OPEN.len()..];
                match after.find(CLOSE) {
                    Some(close) => rest = &after[close + CLOSE.len()..],
                    // Unterminated block — the renderer discards it at flush; drop the rest.
                    None => break,
                }
            }
            None => {
                out.push_str(rest);
                break;
            }
        }
    }
    out
}

/// Append one transport read to `buf` as UTF-8, holding back an incomplete
/// trailing sequence in `carry` for the next read.
///
/// `reqwest::Response::chunk` splits the body at arbitrary byte offsets — a
/// chunked-transfer body surfaces as the socket reads land, and h2 DATA frames
/// are cut wherever the server flushed. So one multi-byte character (an em dash,
/// a curly quote, an accented letter, an emoji) routinely straddles two reads.
/// Decoding each read on its own with `String::from_utf8_lossy` replaced BOTH
/// halves with `U+FFFD`, and since a replacement char is legal inside a JSON
/// string the frame still parsed — the mojibake was forwarded to the renderer
/// and persisted as the finished document, with no error anywhere.
///
/// Genuinely invalid bytes (a corrupt transfer, never a provider) still collapse
/// to a single `U+FFFD` and are skipped, so a malformed stream can never stall
/// the loop.
pub(in crate::commands::ai_provider) fn push_utf8(
    buf: &mut String,
    carry: &mut Vec<u8>,
    bytes: &[u8],
) {
    carry.extend_from_slice(bytes);
    loop {
        match std::str::from_utf8(carry) {
            Ok(text) => {
                buf.push_str(text);
                carry.clear();
                return;
            }
            Err(e) => {
                let valid = e.valid_up_to();
                // `valid_up_to()` is by definition a valid UTF-8 prefix.
                buf.push_str(std::str::from_utf8(&carry[..valid]).unwrap_or_default());
                match e.error_len() {
                    // A truly invalid sequence: emit one replacement char, skip
                    // it, and keep decoding the rest of this read.
                    Some(n) => {
                        buf.push(char::REPLACEMENT_CHARACTER);
                        carry.drain(..valid + n);
                    }
                    // An incomplete tail: hold it back for the next read.
                    None => {
                        carry.drain(..valid);
                        return;
                    }
                }
            }
        }
    }
}
