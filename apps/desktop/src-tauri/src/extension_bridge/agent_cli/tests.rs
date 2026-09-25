//! Every test in the old single `agent_cli/tests.rs` suite is now attributed to
//! the sub-module that owns the code under test (R8's out-of-line test
//! layout: the tests of `foo/bar.rs` live in `foo/bar/tests.rs`). What stays
//! here is the shared fixture set those suites all draw on — declared once,
//! never duplicated.

pub(super) mod support;
