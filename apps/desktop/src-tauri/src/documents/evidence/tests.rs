//! Tests for `documents::evidence` — ranking, extraction, section classification and the entry/date
//! shape predicates behind them.
//!
//! Split by topic: [`ranking`] (`rank_bullets`), [`extraction`] (`extract_evidence`),
//! [`role_opening`] (which lines open a role), [`entry_split`] (employer / title / dates),
//! [`date_shapes`], [`section_classify`], [`skills_split`] and [`lexicon_parity`].

use super::dates::{is_date_only, DATE_ONLY_MARKERS};
use super::entry::{salvage_entry_label, split_two_space_label};
use super::*;

mod date_shapes;
mod entry_split;
mod extraction;
mod lexicon_parity;
mod ranking;
mod role_opening;
mod section_classify;
mod skills_split;

const STRUCTURED: &str = "\
Jane Doe
jane@example.com | +49 30 1234567

EXPERIENCE

Senior Engineer | Acme Corp | 2021 - Present
- Shipped Docker containers onto a Kubernetes cluster
- Ran the weekly standup

Backend Developer | Globex | 2018 - 2021
- Built the billing API in Rust

PROJECTS

- Ledger CLI - a Rust tool for double-entry bookkeeping

EDUCATION

BSc Computer Science, TU Berlin
";
