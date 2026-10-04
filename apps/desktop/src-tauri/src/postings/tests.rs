//! Tests for [`PostingsCache`], split by topic: the upsert-by-id `add` and its
//! embedding invalidation ([`add`]), `update_description` ([`update_description`])
//! and the cache lifecycle — clear, generation counter, cluster annotations
//! ([`lifecycle`]). [`support`] holds the fixtures the siblings share. The
//! interaction store has its own tests under `interactions/`.

use super::*;

mod add;
mod lifecycle;
mod support;
mod update_description;
