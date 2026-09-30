//! Split by what each topic exercises: [`stage`] (the generic `Pipeline`/
//! `Stage` machinery), [`completion`] (`complete_json`'s parse/re-ask seam,
//! the wire-request builder, and its bound check), [`completer`]
//! (`Completer` resolution — `from_config`/`from_override`,
//! `low_effort_level` — and its research admission bucket), and [`cache`]
//! (`pipeline::cache::KvCache`).

mod cache;
mod completer;
mod completion;
mod stage;
