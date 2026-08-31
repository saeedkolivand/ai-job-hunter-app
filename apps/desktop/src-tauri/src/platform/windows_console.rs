//! Windows console attach for the agent-CLI mode (issue #1084 PR 1).
//!
//! The release build sets `windows_subsystem = "windows"` (`main.rs`) +
//! `panic = "abort"` (`Cargo.toml`), so a normal GUI launch has NO console and
//! `GetStdHandle(STD_OUTPUT_HANDLE)` returns a NULL handle — `println!` writing
//! to that handle panics, and with `panic = "abort"` that panic is a hard abort
//! with no message. `ajh-tauri agent …` needs a working stdout in TWO shapes:
//!
//! - **The common case (an agent/LLM harness):** the parent process spawns us
//!   with a redirected/piped stdout. That pipe is INHERITED — `GetStdHandle`
//!   already returns a valid handle, and this module must do NOTHING, because
//!   `AttachConsole` would REPLACE that inherited pipe with the parent's own
//!   console buffer instead, silently breaking the one consumer this feature
//!   exists for.
//! - **The manual/interactive case:** the app was launched from an existing
//!   console (`cmd`/PowerShell) with no pipe — `GetStdHandle` returns
//!   NULL/INVALID, and only THEN do we `AttachConsole(ATTACH_PARENT_PROCESS)`.
//!
//! Probing FIRST and attaching only on the NULL/INVALID branch is the whole
//! point of this module — see [`ensure_console_output`]. No further rebind is
//! needed after a successful attach: Rust's own stdout never caches the OS
//! handle, it re-queries `GetStdHandle` on every write specifically so a
//! `SetStdHandle`/`AttachConsole` mid-process is picked up immediately (see
//! `library/std/src/sys/stdio/windows.rs`'s `get_handle` doc, rust-lang/rust#40490).

/// Probe the current stdout handle; attach to the parent console ONLY when
/// nothing valid is already there. A no-op on every other platform (declared
/// here, not `#[cfg]`-gated at each call site) and on the common
/// inherited-pipe case, so callers never need their own `#[cfg]`.
#[cfg(windows)]
pub fn ensure_console_output() {
    use windows::Win32::System::Console::{
        AttachConsole, GetStdHandle, ATTACH_PARENT_PROCESS, STD_OUTPUT_HANDLE,
    };

    // SAFETY: `GetStdHandle` is a pure query of this process's own standard
    // handle table — no pointers passed, no ownership transferred. The
    // windows-rs wrapper itself treats a NULL *or* `INVALID_HANDLE_VALUE`
    // result as `Err` (see `HANDLE::is_invalid`), so `Ok` here means a
    // genuinely usable handle — exactly what a piped/inherited stdout is.
    let has_valid_handle = unsafe { GetStdHandle(STD_OUTPUT_HANDLE) }.is_ok();
    if has_valid_handle {
        return;
    }

    // SAFETY: `AttachConsole` only attaches this process to an existing
    // console buffer already owned by the parent process; it allocates
    // nothing this process must free. A failure (no parent console — e.g.
    // launched fresh from Explorer) is expected and left silent: stdout
    // stays unusable, same as before the probe, and this function never
    // panics either way — the caller degrades to a non-zero exit with no
    // printed JSON, not a crash.
    let _ = unsafe { AttachConsole(ATTACH_PARENT_PROCESS) };
}

/// No-op on every non-Windows platform — stdout is never NULL there (no
/// `windows_subsystem` GUI mode to escape), so there is nothing to probe.
#[cfg(not(windows))]
pub fn ensure_console_output() {}
