//! Run-store pins: round-trip + byte caps ([`storage`]), the closed `phase`
//! vocabulary ([`phase`]), retention ([`retention`]), url-normalized
//! deletion ([`deletion`]), the backup bundle + import hardening
//! ([`import_export`]), and factory reset + migrations ([`lifecycle`]).
//! Shared fixtures in [`support`].

mod deletion;
mod import_export;
mod lifecycle;
mod phase;
mod retention;
mod storage;
mod support;
