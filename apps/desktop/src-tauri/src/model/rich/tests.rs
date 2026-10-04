//! Tests for `rich.rs`, split by topic: line tokenizing into [`RichText`] runs
//! ([`tokenize`]) and the link helpers it is built on — `url_label`, `split_urls`
//! and `display_text` ([`links`]).

use super::*;

mod links;
mod tokenize;
