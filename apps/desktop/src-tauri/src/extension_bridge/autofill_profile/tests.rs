//! Split by topic (R8 relief -- redistributed from the crate-level `test.rs`, this unit's own
//! tests alone exceed the LOC cap): `resolve` covers `resolve_profile`'s consent gate + the
//! wire-key pinning; `extra_links` covers the `AutofillProfile::from_contact` link projection.

mod extra_links;
mod resolve;
