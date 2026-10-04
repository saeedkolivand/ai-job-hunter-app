//! Unit tests for `documents::keywords` — the tokenizer, the language-aware stopword selection, the
//! numeric-token filter and the coverage math.
//!
//! Split by topic: [`language`] (identity vs. stemmer selection), [`tokenizer`], [`coverage`],
//! [`posting`] (markdown / blob) and [`stopwords`] (per-language lists).

use std::collections::HashSet;

use rust_stemmers::{Algorithm, Stemmer};
use whatlang::{detect, Lang};

use super::language::{language_profile, locale_tag_of};
use super::*;

mod coverage;
mod language;
mod posting;
mod stopwords;
mod tokenizer;
