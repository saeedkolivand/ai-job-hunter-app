//! Tests for the pure intent classifier: the real-world text shapes it must survive
//! ([`text_shape`]), the per-phrase classification rules and their precedence
//! ([`classification`]), and the pins on the compiled-in phrase corpus ([`corpus`]).

use super::*;

mod classification;
mod corpus;
mod text_shape;
