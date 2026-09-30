//! Codex backend tests, split by topic: [`parsing`] (stdout → `CliEvent`/
//! `parse_complete`, both dialects) and [`invocation`] (argv/isolation/effort
//! + `codex debug models` discovery).

mod invocation;
mod parsing;
