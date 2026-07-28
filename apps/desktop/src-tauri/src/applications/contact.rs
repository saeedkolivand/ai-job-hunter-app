//! The contact-alias fold — the import-time half of contact unification.
//!
//! `contact_name`/`contact_email` are the canonical primary contact;
//! `recipient_name`/`recipient_email` are deprecated aliases (see
//! [`super::Application`]). Rows already in SQLite were folded once by the
//! `unify_application_contact` migration; an [`super::Application`] arriving
//! from OUTSIDE the store (an imported bundle) is folded here, by the exact same
//! rule. The two implementations MUST stay in lockstep — a divergence means an
//! imported backup ends up with a different contact than the same row migrated
//! in place.
//!
//! **Emptiness must mean the same thing on both sides.** This side uses
//! [`str::trim`], which strips every Unicode `White_Space` char; SQLite's bare
//! `TRIM(x)` strips only U+0020, so the migration passes an explicit charset
//! (`super::migrations::SQL_TRIM_CHARS` — space, TAB, LF, CR, NBSP) to line the
//! two up on every value a real contact field picks up from HTML or a paste.
//!
//! A deliberate, accepted residual: Rust remains a strict SUPERSET for exotic
//! separators (U+2028 LINE SEPARATOR, U+3000 IDEOGRAPHIC SPACE, …). Such a value
//! reads as empty here and as non-empty in SQL, so a bundle carrying one folds
//! while the same row in place does not. Chasing full parity would mean pinning
//! an unbounded `char()` list to whatever Unicode table the current Rust
//! toolchain ships — a worse trap than the gap. These characters do not occur in
//! a scraped or typed contact field, and the failure mode is a contact that is
//! not promoted (visible, recoverable) rather than a wrong one.
//!
//! Split out of [`super`] to keep the store body under the architecture LOC cap
//! (`tests/architecture.rs` R8).

use super::Application;

/// The one-line note that preserves an apply-by-email contact the contact
/// unification would otherwise drop. The `unify_application_contact` migration
/// builds the SAME literal in SQL — keep the two in lockstep.
fn apply_by_email_note(name: &str, email: &str) -> String {
    format!("Apply-by-email: {name} <{email}>")
}

/// Append `line` to `notes` as its own paragraph, preserving whatever the user
/// already wrote. No-op when the exact line is already present (so a re-import
/// can't stack duplicates), mirroring the migration's `instr(...) = 0` guard.
fn append_note(notes: &mut String, line: &str) {
    if notes.contains(line) {
        return;
    }
    if !notes.is_empty() {
        notes.push_str("\n\n");
    }
    notes.push_str(line);
}

impl Application {
    /// Collapse the deprecated `recipient_*` alias pair onto the canonical
    /// `contact_*` pair, then mirror the result back so both names carry the
    /// same value.
    ///
    /// **PAIR-ATOMIC**, exactly like the `unify_application_contact` migration:
    /// the alias pair is promoted as a UNIT and only when the WHOLE canonical
    /// pair is empty. Promoting each field independently would fuse two
    /// different people — person A's name with person B's address — into one
    /// contact that then feeds the apply-by-email `mailto:` sink, i.e. a message
    /// addressed to B under A's name.
    ///
    /// When the canonical pair is occupied by someone else, a genuinely distinct
    /// alias pair is preserved into `notes` rather than dropped: the store no
    /// longer reads the deprecated columns and an export mirrors the canonical
    /// pair, so it could never be recovered otherwise.
    ///
    /// Applied ONLY where an `Application` arrives from outside this store
    /// ([`super::ApplicationStore::import`]). The in-store build sites set both
    /// names from the same resolved value explicitly instead — folding there
    /// would resurrect a just-cleared contact from its own stale mirror.
    ///
    /// **Lockstep ruling — the SQL semantics win.** Promotion additionally
    /// requires `alias_present`, exactly like the migration's second statement
    /// (`… AND (TRIM(recipient_name) <> '' OR TRIM(recipient_email) <> '')`).
    /// This side used to promote on `canonical_empty` ALONE, which diverged on
    /// one input: a blank-but-not-quite-empty canonical pair (`"  "`) facing an
    /// equally blank alias pair. Rust moved the empty alias over, WIPING the
    /// stored whitespace; SQL matched nothing and left the row untouched. SQL
    /// wins because it is the in-place path every existing user's rows already
    /// took — an imported backup must reproduce what is on disk, not a second
    /// opinion — and because promoting an empty alias is a pure no-op write that
    /// can only destroy, never add, information.
    pub(super) fn canonicalize_contact(mut self) -> Self {
        let alias_present =
            !self.recipient_name.trim().is_empty() || !self.recipient_email.trim().is_empty();
        let canonical_empty =
            self.contact_name.trim().is_empty() && self.contact_email.trim().is_empty();
        if canonical_empty && alias_present {
            self.contact_name = std::mem::take(&mut self.recipient_name);
            self.contact_email = std::mem::take(&mut self.recipient_email);
        } else if alias_present
            && (self.recipient_name != self.contact_name
                || self.recipient_email != self.contact_email)
        {
            let line = apply_by_email_note(&self.recipient_name, &self.recipient_email);
            append_note(&mut self.notes, &line);
        }
        self.recipient_name = self.contact_name.clone();
        self.recipient_email = self.contact_email.clone();
        self
    }
}
