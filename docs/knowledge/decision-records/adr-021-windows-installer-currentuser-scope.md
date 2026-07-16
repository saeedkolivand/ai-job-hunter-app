# ADR-021: Windows installer — pinned currentUser scope

Last updated: 2026-06-14

**Status:** Accepted

## Context

Windows installer configuration was ambiguous:

- Legacy **MSI** installers (Tauri 1.x era) were **per-machine** (admin scope, `Program Files`).
- New **NSIS** bundles (semantic-release) shipped without explicit `bundle.windows` config, so NSIS silently defaulted to **per-machine** scope.
- The **auto-updater only applies to NSIS**, not MSI.
- Result: users with legacy MSI installs could not receive updates. Existing per-machine installs would drift out of sync with the updater, and pinned shortcuts would launch stale executables post-update.

## Decision

**Pin NSIS `installMode` to `currentUser` scope** (user-specific install, no admin required) in `tauri.conf.json`:

```json
{
  "bundle": {
    "windows": {
      "wixLanguage": "en-US",
      "nsis": {
        "installMode": "currentUser",
        "webviewInstallMode": "downloadBootstrapper"
      }
    }
  }
}
```

**Rationale:**

- `currentUser` avoids admin prompt and `Program Files` scope drift (per-user `AppData` is stable).
- `downloadBootstrapper` ensures the WebView2 runtime is installed on first launch (isolated to user).
- The **auto-updater targets currentUser scope**, so future NSIS updates will apply cleanly.

**Migration path for existing per-machine users:**
Document in `DEPLOYMENT.md` that per-machine (MSI) users must **perform a clean uninstall + reinstall** to move to currentUser scope. The updater cannot migrate scope. This is a one-time, documented step.

## Consequences

- **Cleaner update path:** new NSIS installs (and all future updates) use `currentUser`, eliminating scope drift.
- **No admin elevation:** users no longer see UAC prompts.
- **Trade-off:** per-machine scope (e.g., shared corporate machines) is no longer supported; each user must install separately. This is acceptable for the target user base (job hunters).
- **One-time migration cost:** existing per-machine users see a migration notice on first launch pointing to DEPLOYMENT.md.
- **WebView2 runtime:** bundled per-user; does not require system-wide installation.

## Related

- `docs/DEPLOYMENT.md` § "Windows Installer Configuration" — user-facing migration guidance.
