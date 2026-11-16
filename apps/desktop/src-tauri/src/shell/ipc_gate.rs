//! The one decision the IPC handler makes before running any command: refuse
//! everything once startup is aborting. Generic over the invoke type so the branch is
//! testable without constructing a Tauri `Invoke`.

/// Refuse `invoke` (and report it handled) when `locked`, else run it.
pub(super) fn gate<I>(
    locked: bool,
    invoke: I,
    refuse: impl FnOnce(I),
    run: impl FnOnce(I) -> bool,
) -> bool {
    if locked {
        refuse(invoke);
        true
    } else {
        run(invoke)
    }
}

#[cfg(test)]
mod tests;
