//! Split by topic (R8 relief — redistributed from the crate-level `import_tests.rs`, this unit's
//! own tests alone exceed the LOC cap): `basic` covers `resolve_applied_check`'s found/not-found/
//! malformed-url paths + `applied_result_reply`'s wire shape; `identity` covers issue #1214's
//! identity-fallback lookup (mirrors `agent_read::tests`'s own identity coverage). The dispatch-
//! classification tests live in `frame_advance::tests::classify_import` instead.

mod basic;
mod identity;
