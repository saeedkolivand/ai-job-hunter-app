# ADR-014: CLI Agent Install via Shell Plugin with Static npm Capability Allowlist

## Status

Accepted (merged in PR #329)

## Context

Users needed a one-click way to install coding agents (Claude Code, Gemini CLI, etc.) into their development environment without leaving the app or copy-pasting terminal commands. The install process involves spawning `npm install -g <package>` — a privileged OS shell action that requires careful security boundaries.

## Decision

Implement one-click CLI agent install via:

1. **`@tauri-apps/plugin-shell`** adapter (not a raw Rust tokio spawn)
   - Provides OS shell access with capability-based authorization
   - Decouples the IPC interface from platform-specific shell invocation

2. **Static capability allowlist** in `apps/desktop/src-tauri/capabilities/default.json`
   - Exactly 3 fixed-arg commands (one per agent)
   - No dynamic argument injection; the agent registry must **match the capability list exactly**
   - A Rust test asserts the invariant (no extras, no missing)

3. **Adapter-only rule**: The shell plugin is imported **only in `tauri-client/namespaces/cliAgents`** (service-hook layer)
   - Isolates platform-specific concerns to the IPC boundary

4. **Install is automated; login is guided**
   - Once npm finishes, the agent version is re-probed via `redetect()`
   - For OAuth-gated agents (e.g. Claude Code), the guide opens their official docs
   - Headless environments can't drive interactive OAuth, so login is manual (documented in the guide)

## Consequences

### Positive

- **Strong security boundary**: Capability allowlist is immutable; no accidental elevation of privilege
- **Cross-platform**: Shell plugin abstracts `npm` vs `npm.cmd` (Windows) detail
- **Invariant enforced by test**: Registry ↔ capability mismatch is caught immediately
- **Clean IPC contract**: `CliAgentsContract` (status, redetect, install) is platform-agnostic

### Tradeoffs

- **Windows npm.cmd caveat**: On Windows, `npm` may need to be invoked as `npm.cmd`; the guide documents this if detection fails
- **No interactive OAuth**: The app cannot drive interactive login flows; users follow the guide for token setup
- **Headless unsupported**: CI/CD environments without a GUI cannot install agents (by design — OAuth requires a browser)

## Implementation Details

- **IPC contract**: `packages/shared/src/ipc/contracts/cliAgents.ts` — `status()`, `redetect()`, `install()`
- **Rust commands**: `apps/desktop/src-tauri/src/commands/cli_agents.rs` — read-only status/redetect
- **Shell spawn**: Rust `CliAgentBackend::install_package()` + `docs_url()` → plugin-shell execute
- **Service hooks**: `apps/desktop/src/renderer/services/use-cli-agents/use-cli-agents.ts` — `useCliAgents()`, `useInstallCliAgent()`
- **UI**: `features/settings/components/ai-settings/CliAgentInstall` (consent modal, streamed console, guide)
- **Session slice**: Integrated into Settings; agent status cached + re-probed after install
- **Capability allowlist**: `apps/desktop/src-tauri/capabilities/default.json` — 3 shell:allow-execute entries
- **Invariant test**: `apps/desktop/src-tauri/tests/` asserts registry ↔ capability parity

See:

- `packages/shared/src/ipc/contracts/cliAgents.ts` — contract definitions
- `apps/desktop/src-tauri/src/commands/cli_agents.rs` — Rust impl
- `features/settings/components/ai-settings/CliAgentInstall` — UI + guide
