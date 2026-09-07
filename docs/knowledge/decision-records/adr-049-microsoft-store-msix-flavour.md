# ADR-049 — Microsoft Store MSIX as a runtime-detected flavour of the same binary, unvirtualized on purpose

**Status:** Accepted

**Date:** 2026-09-07

**Deciders:** owner (all calls below, including accepting the restricted-capability risk), main session (implementation)

## Context

[ADR-047](adr-047-relicense-to-apache-2-0.md) relicensed the repo so the free OSS code-signing programmes would apply, because the Microsoft Store refuses an unsigned EXE/MSI. An **MSIX** removes that dependency entirely for the Store track: the Store signs the package during submission, so the package this repo produces is unsigned by design.

That raises the real question, which is not packaging but **behaviour**. Windows already runs this app from an NSIS per-user install ([ADR-021](adr-021-windows-installer-currentuser-scope.md)), and three things that install does are wrong inside a package:

- **Updating.** The GitHub updater downloads and runs the NSIS installer. A packaged install that auto-updated would end up with a second, unmanaged copy of the app beside the packaged one.
- **Publishing its own exe path.** Inside a package, `current_exe()` is a `WindowsApps` path that a normal user cannot execute from and whose name carries the package **version** — so everything that records it (the browser native-messaging host of [ADR-015](adr-015-extension-bridge-websocket-save-origin.md), the agent-CLI pointer file of [ADR-037](adr-037-agent-cli-as-binary-mode-thin-client.md)/[ADR-038](adr-038-agent-cli-full-parity-two-tier.md)) dangles after the next Store update.
- **Registering things the manifest owns.** A packaged app's protocol registration, PATH entry and launch-at-login are manifest declarations; writing them at runtime as well is a conflict, not redundancy.

Underneath all three sits one choice with real consequences: whether the package's writes are **virtualized** (redirected into the package's private store) or real.

## Decision

**Ship the Microsoft Store build as an MSIX that wraps the very same `ajh-tauri.exe` the NSIS installer ships** — a container, not a build variant, with no compile-time flag and no second binary. Which container is running is decided **at runtime** by `platform::msix::is_packaged()`, from package identity **plus** the running exe living inside the package install root (identity alone is inherited by child processes, so it cannot tell the two apart on its own); an unreadable probe counts as **packaged**, because the dangerous direction is a packaged build concluding it is unpackaged and re-enabling the GitHub updater. Updating is handed to the Store: the packaged build never checks, never polls, and answers the updater contract with an additive `managedBy` marker that older clients still read as "no update" (`packages/shared/src/ipc/contracts/updater.ts`), while `download`/`install` refuse as defence in depth. **The manifest disables registry and file-system write virtualization** — which is what the restricted `unvirtualizedResources` capability buys — so the HKCU native-messaging registration and the app data directory stay the same real, shared locations a non-Store install uses and a user can move between flavours without losing data; the corollary is that the packaged build must not re-register what the manifest already declares, and that the small set of behaviours which differ is closed and enumerated in `docs/DEPLOYMENT.md` § Microsoft Store (MSIX). Identity values are assigned by Partner Center and therefore supplied as GitHub repository **variables**, never committed, and the unsigned `.msix` is a workflow **artifact** only — never a GitHub Release asset. The first submission is manual; `msstore` automation waits for an Entra tenant to exist.

## Alternatives considered

1. **Ship the existing EXE/MSI on the Store's classic-app track.** Rejected on cost, not fit: that track requires an Authenticode-signed installer, which the project cannot obtain for free from Germany (Azure Trusted Signing's individual tier is US/Canada-only — see [ADR-047](adr-047-relicense-to-apache-2-0.md)). MSIX moves signing to Microsoft.
2. **A compile-time feature flag for the Store build.** Rejected: it produces two binaries that must be built, tested, signed and shipped separately, and it makes "is this a Store install?" unanswerable in a crash report from the field. Runtime detection keeps one artifact and one answer.
3. **Leave write virtualization enabled (a plain MSIX).** Rejected as a first choice because it breaks the extension bridge's native-messaging path — browsers read the real hive, not the package's private copy — and it splits the user's database in two across flavours. It is kept as the primary **fallback** (see below).
4. **The `winapp` CLI for packaging.** Rejected as experimental; `makeappx` from the Windows SDK is the stable, documented path and is what the packer drives.
5. **A community `tauri-windows-bundle` plugin.** Rejected: an unowned third-party dependency in the release path, for a manifest this repo needs to hand-tune anyway.

## Consequences

### Positive

- **One binary, one behaviour, two containers.** Everything not in the enumerated difference list is identical by construction rather than by discipline, and every branch of the detection logic is unit-tested off-Windows because the decision is a pure function of the probe result and the exe path.
- **Data and registrations are shared with the NSIS install**, so switching flavours is not a migration.
- **The Store track no longer waits on code signing.** It is the one distribution channel that never needed [ADR-047](adr-047-relicense-to-apache-2-0.md)'s signing programmes to land.
- **Nothing ships until the listing exists.** The packaging step is skipped while the identity variables are unset, so this rode into `main` ahead of Partner Center.

### Tradeoffs

- **`unvirtualizedResources` is a real Store-rejection risk, accepted knowingly.** Microsoft documents the capability as intended for a narrow partner scenario and "not intended to be used for other scenarios", and it requires a written justification in the submission. The owner kept it because the alternative breaks the extension bridge and splits user data. **If it is rejected**, the recorded fallbacks are (a) a plain virtualized MSIX with the extension reaching the app over the loopback WebSocket only, or (b) a Desktop-Bridge breakaway child process that writes the HKCU keys from outside the container — both of which still need the manifest's startup task for launch-at-login, so that part of this decision survives either way.
- **Two registration mechanisms now exist for the same three concerns** (protocol, CLI reachability, launch-at-login) and they must not both fire. The guards are small, and each is a silent failure if removed — a packaged build re-registering the protocol writes a real, version-pinned, uninstall-surviving key.
- **Every path a packaged build publishes about itself is a decision.** `current_exe()` is the wrong answer there; the execution-alias shim is the stable one. Any future feature that records this app's own path must ask which it needs.
- **WebView2 is a certification risk of its own.** The package cannot run the Evergreen bootstrapper the NSIS installer uses, so a clean Windows 10 test machine without the runtime launches into a dead webview — which is what a certification tester sees. It is handled in the submission's tester notes, not in code.
- **Parts of this cannot be proven in CI.** Native messaging, the CLI alias and launch-at-login all need a genuinely registered package; `docs/DEPLOYMENT.md` names them as a manual checklist and they stay unverified until someone runs it.
- **Uninstalling the package does not remove what the app itself wrote** outside it — the direct, intended consequence of disabling write virtualization, and the same behaviour the NSIS build already has.

## References

- `docs/DEPLOYMENT.md` § "Microsoft Store (MSIX)" — the operational page: the enumerated behaviour differences, identity variables, the local test loop, and the manual submission steps. This record is the _why_; that page is the _how_.
- `apps/desktop/src-tauri/src/platform/msix.rs` — the detection, the published path and the startup task, each with the reasoning at the symbol.
- `apps/desktop/src-tauri/windows/msix/AppxManifest.xml` — the manifest template, commented element by element; `apps/desktop/scripts/pack-msix.mjs` packs it.
- `apps/desktop/src-tauri/src/updater/mod.rs` · `packages/shared/src/ipc/contracts/updater.ts` — the Store-managed updater path and its wire shape.
- [ADR-021](adr-021-windows-installer-currentuser-scope.md) — the NSIS install scope this flavour sits beside · [ADR-015](adr-015-extension-bridge-websocket-save-origin.md) — the bridge whose HKCU registration forces the virtualization call · [ADR-037](adr-037-agent-cli-as-binary-mode-thin-client.md)/[ADR-038](adr-038-agent-cli-full-parity-two-tier.md) — the CLI surface and pointer file the alias path serves · [ADR-047](adr-047-relicense-to-apache-2-0.md) — the signing constraint that started the Store work.
