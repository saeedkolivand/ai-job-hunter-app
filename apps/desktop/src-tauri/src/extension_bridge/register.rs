//! Native-messaging host registration — writes the per-browser host manifests
//! and (on Windows) the HKCU registry pointers so Firefox/Chrome can find and
//! spawn our relay ([`super::native_host`]).
//!
//! Called best-effort from the Tauri `setup` on every launch ([`register_native_host`]):
//! idempotent and overwriting, so `path` (= the current exe) tracks app moves and
//! updates. NEVER panics or propagates to boot — every step logs a warning on
//! failure and continues.
//!
//! ## What gets written
//! A host manifest is a small JSON file naming the host, its `stdio` type, the
//! absolute exe `path`, and the browser-specific allow-list:
//! - **Firefox** uses `"allowed_extensions": ["<gecko-id>"]`.
//! - **Chrome**  uses `"allowed_origins": ["chrome-extension://<id>/"]` (Chrome
//!   requires the trailing slash).
//!
//! Placement is OS- + browser-specific (see [`register_native_host`]). On Windows
//! the browser finds the manifest via an HKCU registry value; on macOS/Linux it
//! reads a fixed well-known directory directly (no registry). The manifest JSON +
//! generic disk-write helpers live in the sibling [`manifest`] module; the per-OS
//! placement itself lives in [`register_windows`]/[`register_unix`] (R8 relief).

use std::path::Path;

use serde_json::json;

use self::manifest::manifest_json;
use self::manifest::write_manifest;

/// Write the agent-CLI pointer file (issue #1084 PR 1) — `{ exePath, dataDir }`
/// — so a separately-invoked `ajh-tauri agent …` process can find both. That
/// process has no `AppHandle` and never inherits `AJH_DATA_DIR` (`set_var`
/// scopes to this process only — see `platform::config::
/// resolve_and_export_data_dir`'s doc), and its own AppHandle-free
/// `data_dir()` fallback (`<home>/.ajh`) is not necessarily where Tauri
/// actually resolved the data dir, so the CLI cannot reconstruct either path
/// on its own. OS- and browser-independent (unlike the manifests below), so
/// this call is unconditional. Rides [`register_native_host`]'s own
/// best-effort/idempotent every-launch lifecycle — see that function's doc —
/// rather than a separate hook: overwritten on every call, so a moved install
/// or a changed data dir is picked up on the very next launch.
///
/// **Resolves `exePath` itself rather than taking it.** The value is the path
/// a HUMAN types, which inside an AppImage is not `current_exe()` — and when
/// the caller passed that choice IN, the choice was the one thing no test
/// covered: reverting the call site to the raw `current_exe()` left every
/// test green. With the resolver inside, the branch that publishes an
/// AppImage path is a property of THIS function and is tested through it. The
/// browser manifests below still take `exe` explicitly, because a
/// native-messaging host is launched by the browser rather than typed, and
/// that difference is the whole point.
fn write_agent_pointer(data_dir: &Path) {
    let Some(path) = crate::platform::config::agent_pointer_path() else {
        log::warn!("[native_host] home dir unavailable — skipping agent-CLI pointer");
        return;
    };
    let Some(exe) = crate::platform::config::agent_cli_exe_path() else {
        log::warn!("[native_host] exe path unavailable — skipping agent-CLI pointer");
        return;
    };
    let pointer = json!({
        "exePath": exe.to_string_lossy(),
        "dataDir": data_dir.to_string_lossy(),
    });
    write_manifest("agent-cli pointer", &path, pointer.to_string().as_bytes());
}

/// Register the native-messaging host for Firefox + Chrome. Best-effort and
/// idempotent — safe to call on every launch.
pub fn register_native_host(data_dir: &Path) {
    let sandboxed = crate::platform::snap::is_packaged();
    register_native_host_inner(data_dir, sandboxed);
}

/// [`register_native_host`], with the Snap check taken as a parameter rather
/// than a call — same split as `updater::store_managed` takes `packaged:
/// bool` — so the guard clause below is exercised by a plain unit test
/// instead of needing to race `snap`'s process-cached `is_packaged()`
/// against env-var mutation.
fn register_native_host_inner(data_dir: &Path, sandboxed: bool) {
    // The pointer publishes the path a HUMAN types to reach the agent CLI,
    // which inside an AppImage is NOT `current_exe()`. It resolves that
    // itself (one resolver, `platform::config::agent_cli_exe_path`, shared
    // with the Settings card behind `commands::system::system_agent_cli_info`,
    // so the file and the UI can never disagree) — see its doc for why the
    // choice is not made here.
    //
    // Written BEFORE the sandbox guard below, deliberately — its own doc
    // says "OS- and browser-independent... this call is unconditional", and
    // that invariant holds even inside Snap: the pointer lands under the
    // CONFINED `$HOME`, which is writable and is exactly what an in-sandbox
    // `snap run ai-job-hunter.agent-cli … agent mcp` invocation reads.
    // Skipping it here would misdiagnose that in-sandbox call as
    // `app_not_located` (`extension_bridge::agent_cli` warns about exactly
    // that class of mistake elsewhere). A HOST-side MCP client still can't
    // reach the confined HOME either way — writing the pointer changes
    // nothing for that case, it only keeps the in-sandbox one working.
    write_agent_pointer(data_dir);

    // Snap confinement has no clean answer for the BROWSER-spawned
    // native-messaging host below: the browser (running OUTSIDE the sandbox)
    // has to spawn this host, but a sandboxed process cannot register a path
    // the browser could launch, and Snap's strict confinement has no escape
    // hatch for it. So this is a disclosed limitation, not silently broken:
    // see docs/DEPLOYMENT.md (Snap Store section).
    if sandboxed {
        log::warn!(
            "[native_host] running inside a Snap sandbox — browser native-messaging \
             registration is not supported in this confinement (the agent-CLI pointer was \
             still written), see docs/DEPLOYMENT.md (Snap Store section)"
        );
        return;
    }
    // A Store (MSIX) build is the one case where `current_exe()` is the WRONG
    // thing to record: it points inside `…\WindowsApps\<PackageFullName>\`,
    // which a normal user — and therefore the browser process that has to spawn
    // this host — cannot execute from, and whose name carries the package
    // VERSION, so the manifest would dangle after the next Store update. The
    // execution-alias shim is the stable, launchable path; `published_exe_path()`
    // answers `Unpackaged` on every other build, where `current_exe()` stays
    // correct, and `Unavailable` when the shim is missing, in which case nothing
    // is published at all.
    let exe = match crate::platform::msix::published_exe_path() {
        crate::platform::msix::PublishedExe::Alias(alias) => alias,
        // A packaged build with no usable alias (the user can switch one off
        // in Settings ▸ Apps ▸ App execution aliases) has NO path worth
        // publishing, so the browser-manifest registration is skipped. The
        // pointer call above ran, but its resolver (`agent_cli_exe_path()`)
        // answers `None` on this same arm, so nothing was actually
        // published there either (see its doc, and
        // `docs/knowledge/agent-cli.md`) — writing `current_exe()` instead
        // would register a host the browser cannot launch. Deliberately not
        // DELETING the existing manifests either: they are shared with a
        // non-Store install on the same machine, which may still own a
        // working one.
        crate::platform::msix::PublishedExe::Unavailable => return,
        crate::platform::msix::PublishedExe::Unpackaged => match std::env::current_exe() {
            Ok(p) => p,
            Err(e) => {
                log::warn!("[native_host] current_exe() failed (non-fatal): {e}");
                return;
            }
        },
    };
    // The agent-CLI pointer was already written above (unconditional,
    // regardless of what `exe` resolves to here). The browser manifests
    // below take `exe` instead of that resolver's path, which differs from
    // it only on Linux/AppImage: a native-messaging host is launched by the
    // browser, not typed.
    let firefox_json = manifest_json(&exe, true);
    let chrome_json = manifest_json(&exe, false);

    #[cfg(windows)]
    win::register_windows(data_dir, &firefox_json, &chrome_json);
    #[cfg(not(windows))]
    unix::register_unix(data_dir, &firefox_json, &chrome_json);
}

mod manifest;
#[cfg(not(windows))]
mod unix;
/// Windows placement (HKCU registry) — named `win` (not `windows`) so this module never
/// shadows the `windows` crate its own `use` statements need.
#[cfg(windows)]
mod win;

#[cfg(test)]
mod tests;
