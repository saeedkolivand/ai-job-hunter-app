//! Shared fixture for `agent_call::validate::tests`' topic modules — a real command's presence
//! in the GENERATED `CATALOGUE`, the same "close the gap between the general logic and the real
//! table" discipline `agent_call::proof::tests` already uses for `POLICY`.

use super::super::*;

pub(super) fn has_command(command: &str) -> bool {
    CATALOGUE.iter().any(|e| e.command == command)
}
