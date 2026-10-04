//! Shell wiring for `lib.rs::run()`, split out of it for R8 (issue #1280): the panic
//! hook, the deep-link plumbing, the `setup` hook (with its managed state) and the
//! IPC command registry. L3 — the same layer as `lib`: it assembles the app and is
//! never reached from below. The plugin chain and the `run()` entry itself stay in
//! `lib.rs`.

mod deep_link;
mod handler;
mod panic_hook;
mod setup;
mod state;

pub(crate) use deep_link::{handle_deep_link, log_rejected_deep_link};
pub(crate) use handler::invoke_handler;
pub(crate) use panic_hook::install_crash_log_hook;
pub(crate) use setup::setup;
