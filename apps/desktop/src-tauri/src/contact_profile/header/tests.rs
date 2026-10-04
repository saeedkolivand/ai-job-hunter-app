//! Tests for the header contact line, split by the question each answers: what
//! `header_markdown` renders and sanitizes ([`markdown`]), that `header_urls`
//! stays in lockstep with it ([`urls`]), and how `apply_to_header` falls back to
//! the profile ([`apply`]).

use super::*;
use crate::contact_profile::{ContactLink, LocalizedText};

mod apply;
mod markdown;
mod urls;
