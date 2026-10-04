//! Tests for the RFC 8601 `Authentication-Results` tokeniser, split by what they pin: the shapes
//! that must still authorise ([`legitimate`]), and everything that must fail closed or resolve to
//! the real verdict — truncation, injection, malformed structure ([`fail_closed`]).

use super::*;

mod fail_closed;
mod legitimate;
