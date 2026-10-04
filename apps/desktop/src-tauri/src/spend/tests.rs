//! Store tests for [`SpendStore`], split by topic: the record/list/aggregate/backup
//! round-trip ([`store`]), the reported thinking-token column ([`thinking`]) and
//! window scoping ([`window`]). [`support`] holds the fixtures the siblings share.
//! The rate table has its own tests under `rates/`.

use super::*;

mod store;
mod support;
mod thinking;
mod window;
