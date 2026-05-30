//! Shared subprocess helpers.
//!
//! On Windows, spawning a console subprocess flashes a transient console window;
//! `CREATE_NO_WINDOW` suppresses it. Every subprocess we spawn calls
//! [`NoWindow::no_window`] so this behavior is defined in exactly one place — new
//! spawn sites opt in with a single call. It is a compile-time no-op on macOS and
//! Linux, which don't show a window for a spawned child.

/// Adds `.no_window()` to the `Command` builders we use (`std` and `tokio`).
/// Returns `&mut Self` so it chains inside an existing builder expression.
pub trait NoWindow {
    /// Suppress the transient console window Windows shows when spawning a console
    /// subprocess. No-op on non-Windows.
    fn no_window(&mut self) -> &mut Self;
}

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

impl NoWindow for std::process::Command {
    fn no_window(&mut self) -> &mut Self {
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            self.creation_flags(CREATE_NO_WINDOW);
        }
        self
    }
}

impl NoWindow for tokio::process::Command {
    fn no_window(&mut self) -> &mut Self {
        #[cfg(windows)]
        {
            // tokio's Command exposes `creation_flags` inherently on Windows.
            self.creation_flags(CREATE_NO_WINDOW);
        }
        self
    }
}
