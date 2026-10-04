//! Tests for the hybrid-search ranking arms, split by topic: dense similarity and
//! rank fusion, which are pure ([`rank`]); and the FTS5-backed lexical arm in its
//! implicit-AND mode ([`lexical`]) and its OR mode, `search_any` ([`search_any`]).
//! [`support`] holds the fixtures the two lexical files share.

use super::dense::{cosine, rank_by_similarity};
use super::fusion::reciprocal_rank_fusion;
use super::lexical::{LexicalDoc, LexicalIndex};

mod lexical;
mod rank;
mod search_any;
mod support;
