//! Fixtures shared by the stage-override tests.

use super::*;

pub(super) fn over(provider: &str, model: &str) -> StageOverride {
    StageOverride {
        provider: provider.to_string(),
        model: model.to_string(),
        context_window: None,
    }
}
