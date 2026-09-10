# ADR-049 — Packaged builds (MSIX, Snap, Flatpak) as runtime-detected flavours of the same binary

**Status:** Accepted

**Date:** 2026-09-07 · **Amended:** 2026-09-10 to cover Snap Store and Flathub alongside MSIX

**Deciders:** owner (all calls below, including accepting the restricted-capability risk), main session (implementation)

## Context

This decision applies to **three distribution containers** — each of which raises the same core question about **behaviour** rather than packaging format:

- [ADR-047](adr-047-relicense-to-apache-2-0.md) relicensed the repo so the free OSS code-signing programmes would apply for Windows distribution. The Microsoft Store's **MSIX** removes that signing dependency entirely: the Store signs the package during submission, so the package this repo produces is unsigned by design.
- Linux distributions on Snap Store and Flathub have their own policy: the package must be built and submitted through their respective channels, neither of which allows a second bare-binary installer alongside it.
- All three — MSIX, Snap, and Flatpak — share a common problem: three things that a bare installer does are wrong inside a container.

That real question is **behaviour**, not packaging format. The app already runs from an NSIS per-user install on Windows ([ADR-021](adr-021-windows-installer-currentuser-scope.md)); on Linux it shipped as AppImage/DEB. Three things that bare install does are wrong inside a package:

- **Updating.** The GitHub updater downloads and runs the bare installer (NSIS, AppImage, or the equivalent). A packaged install that auto-updated would end up with a second, unmanaged copy of the app beside the packaged one.
- **Publishing its own exe path.** Inside a package, `current_exe()` is either a version-stamped path (MSIX's `WindowsApps` path) or a confined path (`$SNAP` on Snap, or under the Flatpak mount) whose name or location is owned by the container — so everything that records it (the browser native-messaging host of [ADR-015](adr-015-extension-bridge-websocket-save-origin.md), the agent-CLI pointer file of [ADR-037](adr-037-agent-cli-as-binary-mode-thin-client.md)/[ADR-038](adr-038-agent-cli-full-parity-two-tier.md)) dangles after the next container update or rebase.
- **Registering things the manifest owns.** Each container's protocol registration, PATH entry and launch-at-login are manifest declarations (MSIX's `AppxManifest.xml`, Snap/Flatpak's `snapcraft.yaml`/`metainfo.xml`); writing them at runtime as well is a conflict, not redundancy.

Underneath all three sits one choice with real consequences: whether the container's writes are **virtualized** (redirected into a private store) or real, and how the app discovers which container holds it.

## Decision

**Ship each packaged distribution (MSIX on Windows, Snap and Flatpak on Linux) as a container wrapping the very same `ajh-tauri` binary the bare installer ships** — containers, not build variants, with no compile-time flag and no second binary. Which container is running is decided **at runtime**:

- **Windows (MSIX):** `platform::msix::is_packaged()` checks package identity **plus** the running exe living inside the package's install root (identity alone is inherited by child processes, so it cannot tell the two apart; both sides are canonicalized first, since neither short names nor Windows case rules survive a string compare).
- **Linux (Snap):** `platform::snap::is_packaged()` checks whether `current_exe()` lives under the `$SNAP` directory.
- **Linux (Flatpak):** `platform::flatpak::is_packaged()` checks for the `/.flatpak-info` file.

Missing or unreadable evidence counts as **not packaged**, because the dangerous direction is a packaged build concluding it is unpackaged and re-enabling the GitHub updater. An aggregator `platform::is_packaged_build()` and the enum `PackageFlavour` (`Msstore`, `Snap`, `Flatpak`) summarize all three.

Each behaviour that differs is owned by its own symbol rather than by this record. The set is closed:

- **Updating.** Each packaged build never checks and never polls for updates — the container's store owns updating. The updater contract's marker, present on the wire to let older clients read "no update", and the refusals behind `download`/`install` live in `apps/desktop/src-tauri/src/updater/mod.rs` and `packages/shared/src/ipc/contracts/updater.ts`.
- **What it publishes about itself.** MSIX publishes the execution-alias shim (and nothing if the alias is disabled). Snap/Flatpak's path publishing is confined to the container's own mount, and is not published outside the sandbox. Details: `platform::msix::published_exe_path` (Windows), and the Snap/Flatpak cases in `platform::config::agent_cli_exe_path` (shared by both). When no usable path exists (MSIX with no alias), the native-messaging registration and the agent-CLI pointer are skipped rather than written with an unusable path — and not deleted, since they may belong to a non-packaged install on the same machine.
- **Native-messaging host registration.** A packaged Snap or Flatpak build skips registration entirely because the sandbox has no usable solution for the browser to reach the host process — even with breakaway processes (Flatpak's `flatpak-spawn --host` pattern breaks isolation by design; Snap has no equivalent escape hatch at all). MSIX, with write virtualization disabled, can register normally. The guard and its warning live in `apps/desktop/src-tauri/src/extension_bridge/register.rs`.
- **Launch at login.** MSIX delegates to the manifest's `StartupTask` (awaited WinRT calls, can be refused by the user or policy). Snap/Flatpak delegate to the manifest's `autostart` interface (Background portal for Flatpak, `autostart` scope for Snap).
- **Write virtualization (MSIX only).** Write virtualization is deliberately disabled — which is what the `unvirtualizedResources` restricted capability the manifest declares buys — so the HKCU native-messaging registration and the app data directory stay the same real, shared locations a non-packaged install uses, and a user can move between flavours without losing data. The corollary is that the packaged build must not re-register what the manifest already declares. Snap/Flatpak have their own permissions model (plugs/interfaces); this consequence does not apply.

Identity values for MSIX are assigned by Partner Center and supplied as GitHub repository **variables**, never committed. The unsigned `.msix` is a workflow **artifact** only — never a GitHub Release asset. Submission is automated by the `publish-msstore` job via the `msstore` CLI. Snap and Flatpak submissions follow their respective platform processes (documented in `docs/DEPLOYMENT.md`). The set of behaviours that differ is closed and enumerated at `platform::*` modules themselves, with `docs/DEPLOYMENT.md` § Snap Store / § Flathub / § Microsoft Store (MSIX) as the operational view of each.

## Alternatives considered

**Per-platform alternatives:**

1. **Windows / MSIX**:
   - **Ship the existing EXE/MSI on the Store's classic-app track.** Rejected on cost, not fit: that track requires an Authenticode-signed installer, which the project cannot obtain for free from Germany (Azure Trusted Signing's individual tier is US/Canada-only — see [ADR-047](adr-047-relicense-to-apache-2-0.md)). MSIX moves signing to Microsoft.
   - **A compile-time feature flag for the Store build.** Rejected: it produces two binaries that must be built, tested, signed and shipped separately, and it makes "is this a Store install?" unanswerable in a crash report from the field. Runtime detection keeps one artifact and one answer.
   - **Leave write virtualization enabled (a plain MSIX).** Rejected as a first choice because it breaks the extension bridge's native-messaging path — browsers read the real hive, not the package's private copy — and it splits the user's database in two across flavours. It is kept as the primary **fallback** (see Consequences, below).
   - **The `winapp` CLI for packaging.** Rejected as experimental; `makeappx` from the Windows SDK is the stable, documented path.
   - **A community `tauri-windows-bundle` plugin.** Rejected: an unowned third-party dependency in the release path.

2. **Linux / Snap and Flatpak**: Both Snap Store and Flathub were **selected** for their respective ecosystems as the canonical distribution channels; building a second bare Linux installer alongside packaged releases would duplicate tooling and testing without material benefit — the packaged stores provide auto-update, confinement, and user familiarity within their respective user bases.

## Consequences

### Positive

- **One binary, three containers.** Everything not in the enumerated difference list is identical by construction rather than by discipline. Each detection function is a pure function of its probe (package identity + exe location for MSIX; filesystem presence for Snap/Flatpak), unit-tested off its native platform where possible.
- **Data and registrations are shared across flavours (Windows only).** A user can switch between the Microsoft Store (MSIX) and the bare Windows installer without a migration — the app data directory and HKCU registrations stay in the same real locations. On Linux, Snap and Flatpak each maintain their own separate, confined app-data locations; switching between them requires manual data migration (not yet implemented) or rebuilding from scratch.
- **Each distribution track is independent.** The Windows Store track is not blocked on bare-installer signing. Snap/Flatpak are native to their respective Linux distro ecosystems. No packaging track depends on another.
- **Nothing ships to a store until the listing exists.** For MSIX, the packaging step is skipped while Partner Center identity variables are unset. For Snap/Flatpak, submission is manual and happens only when the platform channel exists.

### Tradeoffs

- **(MSIX) `unvirtualizedResources` is a real Store-rejection risk, accepted knowingly.** Microsoft documents the capability as intended for a narrow partner scenario and "not intended to be used for other scenarios", and it requires written justification in submission. The owner kept it because the alternative breaks the extension bridge and splits user data across flavours. **If it is rejected**, recorded fallbacks are (a) a plain virtualized MSIX with the extension reaching the app over loopback WebSocket only, or (b) a Desktop-Bridge breakaway child that writes HKCU keys from outside the container — both still need the manifest's startup task for launch-at-login, so that part survives either way.
- **(Snap/Flatpak) Native messaging is unavailable.** A browser cannot reliably reach the host process inside a strict sandbox, even via breakaway mechanisms — Flatpak's `flatpak-spawn --host` breaks isolation by design; Snap has no equivalent escape hatch at all. The extension bridge works for in-sandbox scenarios (e.g., GNOME Web). This is a disclosed limitation, listed in `docs/DEPLOYMENT.md`.
- **Two (or three) registration mechanisms now exist for the same concerns** (protocol, CLI reachability, launch-at-login) and they must not both fire. The guards are small, and each is a silent failure if removed — a packaged build re-registering the protocol writes a real, version-pinned, uninstall-surviving key.
- **Every path a packaged build publishes about itself is a decision.** `current_exe()` is the wrong answer (version-stamped or confined); the execution-alias shim is stable for MSIX, and confined paths are fine for Snap/Flatpak. "Publish nothing, skip the write" is a real third answer. Any future feature that records this app's own path must ask which it needs.
- **(MSIX) WebView2 is a certification risk.** The package cannot run the Evergreen bootstrapper the NSIS installer uses, so a clean Windows 10 test machine without the runtime launches into a dead webview. This is handled in the submission's tester notes, not in code.
- **(Flatpak) pnpm offline bootstrap is unverified end-to-end.** Flathub's buildbot has no network access; pnpm's registry metadata fetches are mitigated with config overrides confirmed working in WSL testing, but a genuinely clean build inside flathub-builder is not yet proven. See `docs/DEPLOYMENT.md` § Flathub for the known open risk.
- **Parts of this cannot be proven in CI.** Native messaging, the CLI alias, launch-at-login, and the browser bridge all need a genuinely running packaged container; `docs/DEPLOYMENT.md` names them as manual checklists and they stay unverified until someone runs them. This is not unique to packaged builds — it is the tradeoff of runtime detection.
- **Uninstalling a container does not remove what the app itself wrote** outside it (MSIX: HKCU keys and app data; Snap/Flatpak: confined app data). This is the direct, intended consequence of MSIX's disabled virtualization and the same behaviour the bare installer already has on Windows. On Linux, app data always lives outside the container by design.

## References

**Detection and behaviour differences (all platforms):**

- `apps/desktop/src-tauri/src/platform/mod.rs` — `is_packaged_build()` aggregator and `PackageFlavour` enum
- `apps/desktop/src-tauri/src/platform/msix.rs` — Windows MSIX detection, published path, startup task
- `apps/desktop/src-tauri/src/platform/snap.rs` — Linux Snap detection
- `apps/desktop/src-tauri/src/platform/flatpak.rs` — Linux Flatpak detection
- `apps/desktop/src-tauri/src/updater/mod.rs` · `packages/shared/src/ipc/contracts/updater.ts` — packaged-aware updater wire shape
- `apps/desktop/src-tauri/src/extension_bridge/register.rs` — native-messaging guard (skips on Snap/Flatpak)

**Manifests and tooling:**

- `apps/desktop/src-tauri/windows/msix/AppxManifest.xml` — MSIX manifest, with reasoning on every element; packed by `apps/desktop/scripts/pack-msix.mjs`
- `apps/desktop/src-tauri/linux/snap/snapcraft.yaml` — Snap manifest; version synced by `scripts/sync-snapcraft.cjs`
- `apps/desktop/src-tauri/linux/flatpak/io.github.saeedkolivand.AIJobHunter.yml` — Flatpak manifest; version and vendor-sources synced by CI

**Operational documentation:**

- `docs/DEPLOYMENT.md` § "Snap Store" / "Flathub" / "Microsoft Store (MSIX)" — identity variables, local test loops, manual setup and verification checklists for each platform. This record is the _why_; those sections are the _how_.

**Related decisions:**

- [ADR-021](adr-021-windows-installer-currentuser-scope.md) — the NSIS install scope the MSIX flavour sits beside
- [ADR-015](adr-015-extension-bridge-websocket-save-origin.md) — the bridge whose HKCU registration drives the virtualization question
- [ADR-037](adr-037-agent-cli-as-binary-mode-thin-client.md)/[ADR-038](adr-038-agent-cli-full-parity-two-tier.md) — the CLI surface and pointer file the path publishing serves
- [ADR-047](adr-047-relicense-to-apache-2-0.md) — the signing constraint that motivated the Store track
