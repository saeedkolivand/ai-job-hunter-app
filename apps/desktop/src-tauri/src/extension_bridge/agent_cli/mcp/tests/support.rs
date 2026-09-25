//! Fixtures more than one topic under `tests/` reaches for, so they live here instead of in the
//! hub: the writers/readers that let a test watch `serve`’s threads from outside, and the
//! prose-window assertions two `INSTRUCTIONS` topics share. `pub(super)` because the hub
//! re-exports them with `use support::*;`, which is what makes them visible to a topic.

use super::*;
/// [`serve`]'s single writer, instrumented: mirrors every byte into a shared buffer and, the
/// first time that buffer contains `needle`, pulses `signal` exactly once. Lets a dispatch stub
/// running on the WORKER thread block until a frame the MAIN thread emitted has really been
/// written — the only way to observe "answered mid-call" from outside.
pub(super) struct SignallingWriter {
    pub(super) buffer: Arc<Mutex<Vec<u8>>>,
    pub(super) needle: &'static str,
    pub(super) signal: Option<std::sync::mpsc::Sender<()>>,
}

impl Write for SignallingWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let seen = {
            let mut sink = lock(&self.buffer);
            sink.extend_from_slice(buf);
            String::from_utf8_lossy(&sink).contains(self.needle)
        };
        if seen {
            if let Some(tx) = self.signal.take() {
                let _ = tx.send(());
            }
        }
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

/// A writer whose every write fails — the EPIPE a client that closed its pipe produces.
pub(super) struct BrokenWriter;

impl Write for BrokenWriter {
    fn write(&mut self, _buf: &[u8]) -> std::io::Result<usize> {
        Err(std::io::Error::other("client closed the pipe"))
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Err(std::io::Error::other("client closed the pipe"))
    }
}

/// A writer that PARKS inside its FIRST `write` — pulsing `parked` first — until the test drops
/// its release sender, then accepts everything. The client that stopped draining stdout, held
/// still on purpose: while it is parked the whole loop is stuck inside [`emit`], which is the
/// only state in which the reader thread can be observed running ahead of the writer.
pub(super) struct ParkingWriter {
    pub(super) buffer: Arc<Mutex<Vec<u8>>>,
    pub(super) parked: Option<std::sync::mpsc::Sender<()>>,
    pub(super) release: std::sync::mpsc::Receiver<()>,
}

impl Write for ParkingWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        if let Some(tx) = self.parked.take() {
            let _ = tx.send(());
            // Returns as soon as the test drops the sender (Disconnected); the budget is only
            // there so a broken test fails instead of hanging the run.
            let _ = self.release.recv_timeout(SIGNAL_BUDGET);
        }
        lock(&self.buffer).extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

/// A [`BufRead`] that hands out ONE line per `fill_buf` and counts every line it has handed over.
/// A [`Cursor`] cannot answer the question the bound is about — "how far did the reader get before
/// it stopped?" — because it is consumed in whatever chunks the reader asks for; this counts the
/// lines the reader thread actually pulled, so a reader parked on a full queue and a reader that
/// swallowed the entire input are two different numbers.
pub(super) struct PacedInput {
    pub(super) lines: std::vec::IntoIter<String>,
    pub(super) current: Vec<u8>,
    pub(super) pos: usize,
    pub(super) produced: Arc<AtomicUsize>,
}

impl std::io::Read for PacedInput {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let taken = {
            let available = self.fill_buf()?;
            let n = available.len().min(buf.len());
            buf[..n].copy_from_slice(&available[..n]);
            n
        };
        self.consume(taken);
        Ok(taken)
    }
}

impl std::io::BufRead for PacedInput {
    fn fill_buf(&mut self) -> std::io::Result<&[u8]> {
        if self.pos == self.current.len() {
            self.current = self.lines.next().unwrap_or_default().into_bytes();
            self.pos = 0;
            if !self.current.is_empty() {
                // Counted on HAND-OVER, so the count is "lines the reader has begun reading",
                // never "lines the test wrote".
                self.produced.fetch_add(1, Ordering::SeqCst);
            }
        }
        Ok(&self.current[self.pos..])
    }

    fn consume(&mut self, amt: usize) {
        self.pos = (self.pos + amt).min(self.current.len());
    }
}
/// Issue #1170 round-4 review (`B1-r1-ACLI-R4-2`): every generic-tier reply pipes `text` through
/// `agent_call::fence_scraped_fields`, which fences it with `prompt_fence::JOB_CAP` — so a document
/// longer than the cap comes back silently truncated, with no truncation marker on the wire. The
/// old prose promised documents_list rows "already carry the full `text`" on BOTH surfaces below;
/// neither may claim "full" again. The last assertion proves the claim really would be false: text
/// well over the cap comes back shorter than it went in.
///
/// Round 5 (`B1-r1-ACLI-R5-5`): the two negative assertions below only deny the EXACT substrings
/// the round-4 fix happened to write — "rows already carry the complete text", "rows carry the
/// whole document", or "the entire text" would all satisfy both negatives while overclaiming
/// exactly the same thing. Assert the POSITIVE clause on both surfaces too, so a rewrite that
/// drops the caveat (while carefully avoiding the two banned phrases) still fails.
///
/// Round 6 (`B1-r2-ACLI-R6-3`): round 5's positive clause was satisfied by EITHER command's
/// mention — a prose that says "documents_list rows are fenced and capped" once, then separately
/// claims documents_get_text returns the "FULL, uncapped text", passed both assertions unchanged
/// (`fenced and capped` was present; `carry the full`/`full text` were never the phrase actually
/// written). [`assert_document_read_prose_is_honest`] instead: (a) bans "uncapped" anywhere,
/// case-insensitively; (b) bans the STANDALONE word "full" anywhere, not just inside one
/// hand-picked phrase like "carry the full" — "FULL, uncapped text" fails on both grounds now;
/// (c) walks every literal `documents:<cmd>` token found IN the string and requires "capped" to
/// appear near THAT occurrence.
///
/// Round 7 (`B1-r3-ACLI-R7-2`): (c) used the same wide, backward-reaching `window` as (a)/(b), so
/// two `documents:<cmd>` tokens sitting close together (as they do in the real prose) let ONE
/// command's cap disclosure satisfy the OTHER's requirement — the exact failure (c) claims to
/// prevent. The cap check now uses a forward-only span from this token to the NEXT `documents:`
/// occurrence (or the end of the string), so a disclosure written only near a neighbouring
/// command's mention can no longer cover this one. `window` (backward+forward) stays for the
/// full/uncapped bans, which round 6 needs to catch banned words sitting BEFORE the token.
// Byte-safe window bounds — `prose` is human prose with non-ASCII chars (e.g. "résumé"), so an
// arbitrary `idx - 120` can land mid-character; walk to the nearest valid boundary rather than
// panicking on a sliced-through multi-byte char. Module-level (not nested in the test below) so
// `documents_text_prose_per_token_cap_disclosure_is_required` can reuse them against synthetic
// prose without duplicating the window logic.
pub(super) fn floor_char_boundary(s: &str, index: usize) -> usize {
    let mut i = index.min(s.len());
    while i > 0 && !s.is_char_boundary(i) {
        i -= 1;
    }
    i
}
pub(super) fn ceil_char_boundary(s: &str, index: usize) -> usize {
    let mut i = index.min(s.len());
    while i < s.len() && !s.is_char_boundary(i) {
        i += 1;
    }
    i
}

pub(super) fn assert_document_read_prose_is_honest(prose: &str, label: &str) {
    let mut found_any = false;
    for cmd in ["documents_list", "documents_get_text"] {
        let token = format!("documents:{cmd}");
        let Some(idx) = prose.find(&token) else {
            continue;
        };
        found_any = true;
        // A window AROUND the token, not just after it — the round-6 defect's banned words
        // sat BEFORE the token ("for a document's FULL, uncapped text, call-read
        // documents:documents_get_text …"), so an after-only window would have missed it.
        let start = floor_char_boundary(prose, idx.saturating_sub(120));
        let end = ceil_char_boundary(prose, idx + token.len() + 250);
        let window = &prose[start..end];

        // Forward-only, and bounded by the NEXT `documents:` token — so a cap disclosure
        // sitting near a different command's mention (before this token, or past the next
        // one) can never satisfy this command's own requirement.
        let next_token_start = prose[idx + token.len()..]
            .find("documents:")
            .map(|p| idx + token.len() + p)
            .unwrap_or(prose.len());
        let cap_end = ceil_char_boundary(prose, (idx + token.len() + 250).min(next_token_start));
        let cap_window = &prose[idx..cap_end];
        assert!(
            cap_window.contains("capped"),
            "{label}'s mention of `{token}` must disclose a cap near ITS OWN occurrence, \
             not rely on a disclosure written only near a different command's mention: \
             …{cap_window}…"
        );
        assert!(
            !window.to_ascii_lowercase().contains("uncapped"),
            "{label}'s mention of `{token}` must never claim it is uncapped: …{window}…"
        );
        assert!(
            !window
                .split(|c: char| !c.is_ascii_alphabetic())
                .any(|word| word.eq_ignore_ascii_case("full")),
            "{label}'s mention of `{token}` must never claim it returns the FULL text: \
             …{window}…"
        );
    }
    assert!(
        found_any,
        "{label} must name at least one documents:<cmd> read: {prose}"
    );
}
