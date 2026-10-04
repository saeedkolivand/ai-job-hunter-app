//! The panic hook that appends to `crashes.log`. Split out of `lib.rs` for R8
//! (issue #1280); installed by `run()` at the same point as before.

/// Install the crash-reporter panic hook before everything else so panics
/// that occur during setup are also caught.  We chain the previous hook
/// (the default) so stderr still prints as usual.
///
/// Kept alongside Sentry rather than replaced by it: `crashes.log` is the
/// offline record the user can export from Settings (ADR-027), and it must
/// keep working when reporting is switched off or the machine is offline.
pub(crate) fn install_crash_log_hook() {
    let prev_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        // Chain default hook first — keeps the familiar stderr output intact.
        prev_hook(info);
        // Best-effort append to crashes.log; ignore every IO error so we never
        // panic inside the panic hook.
        let _ = (|| -> std::io::Result<()> {
            use std::io::Write as _;
            let log_path = crate::platform::config::data_dir().join("crashes.log");
            if let Some(dir) = log_path.parent() {
                let _ = std::fs::create_dir_all(dir);
            }
            let mut file = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&log_path)?;
            let timestamp = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ");
            let msg = info
                .payload()
                .downcast_ref::<&str>()
                .copied()
                .or_else(|| info.payload().downcast_ref::<String>().map(String::as_str))
                .unwrap_or("<non-string panic payload>");
            let location = info
                .location()
                .map(|l| format!("{}:{}:{}", l.file(), l.line(), l.column()))
                .unwrap_or_else(|| String::from("<unknown>"));
            // Rendered to a String BEFORE touching the file, and written in one
            // call. Formatting a `Backtrace` directly into the file writer means
            // a failure part-way through leaves a half-written entry on disk —
            // which is what a reported bundle contained: two PANIC entries that
            // stopped dead after "Backtrace:", with no frames and not even the
            // `---` terminator, so neither crash could be diagnosed. This panic
            // fires during shutdown, when a partial write is exactly what you
            // would expect.
            let bt = std::backtrace::Backtrace::force_capture().to_string();
            let bt = if bt.trim().is_empty() {
                // Say so explicitly. A blank section is indistinguishable from a
                // truncated write, and that ambiguity is what cost the last two.
                "<backtrace unavailable — capture returned no frames>".to_string()
            } else {
                bt
            };
            let entry =
                format!("[{timestamp}] PANIC at {location}: {msg}\nBacktrace:\n{bt}\n---\n");
            file.write_all(entry.as_bytes())?;
            file.flush()?;
            Ok(())
        })();
    }));
}
