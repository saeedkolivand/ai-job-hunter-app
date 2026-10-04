//! Tests for [`SalaryResearch`], split by topic: the pure helpers and the strict
//! validation of a provider response ([`validation`]), and `enrich` driven through
//! fake searchers against a real [`KvCache`] ([`enrich`]). [`support`] holds the
//! fixtures the two share.

use super::*;

mod enrich;
mod support;
mod validation;
