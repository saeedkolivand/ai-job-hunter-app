//! The launch tier itself — the one place raw `--allow-*` flags become a type. Kept apart
//! from the transport and the protocol layer because every other unit takes a `Tier` and none
//! of them is allowed to re-derive it from a raw bool pair.

/// The three strictly-nested launch tiers this server can run at (MEDIUM fix, security review
/// round 3 — item 21): replaces a raw `(allow_reversible, allow_irreversible)` bool pair that let
/// `Server::new(false, true)` compile and pass every existing test even though no real launch can
/// ever produce it (`--allow-irreversible` alone always implies the reversible tier too). The
/// type itself makes that state unconstructable, rather than a "callers MUST resolve the
/// implication first" comment on every function that used to take the raw pair.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Tier {
    Read,
    Reversible,
    Irreversible,
}

impl Tier {
    /// The ONE place raw launch flags become a [`Tier`] — `--allow-irreversible` implies
    /// `--allow-reversible` here, once.
    pub(super) fn from_flags(allow_reversible: bool, allow_irreversible: bool) -> Self {
        if allow_irreversible {
            Tier::Irreversible
        } else if allow_reversible {
            Tier::Reversible
        } else {
            Tier::Read
        }
    }

    pub(super) fn allows_reversible(self) -> bool {
        matches!(self, Tier::Reversible | Tier::Irreversible)
    }

    pub(super) fn allows_irreversible(self) -> bool {
        matches!(self, Tier::Irreversible)
    }
}
