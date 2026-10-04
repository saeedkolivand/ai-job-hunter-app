//! Scrape IPC commands. The bodies live in one submodule per responsibility (R8,
//! issue #1280) and are glob re-exported here: a `#[tauri::command]` also generates
//! sibling macros the `generate_handler!` list looks up beside the function, and a
//! glob carries those along, so every command keeps its `commands::scrape::<name>`
//! path — the path the handler list and the agent-CLI policy table name it by.

mod clusters;
mod interactions;
mod postings;
mod run;

pub use clusters::*;
pub use interactions::*;
pub use postings::*;
pub use run::*;

// ScrapeBoardsRequest and ScrapeUrlRequest are generated from the Zod schemas in
// packages/shared by `pnpm gen:ipc`. See crate::ipc_contracts::scrape.
pub use crate::ipc_contracts::scrape::{ScrapeBoardsRequest, ScrapeUrlRequest};
