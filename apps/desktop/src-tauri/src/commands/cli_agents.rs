//! CLI-agent install status (#22). Read-only: reports which coding-agent CLIs are
//! installed (cached `<binary> --version` probe), their npm package + docs URL for
//! the in-app install/guide UI, and whether `npm` is available to drive a one-click
//! install. The install spawn itself runs through the shell plugin
//! (capability-scoped, fixed args) on the renderer side — never here. This module
//! only reads status; it spawns nothing but the existing detection probe.

use serde::Serialize;

use crate::commands::ai_provider::cli_agent;

/// Per-agent install status for the Settings → AI "CLI agents" panel.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CliAgentStatus {
    /// Provider id, e.g. `claude-code` / `codex` / `gemini-cli`.
    pub id: String,
    /// Binary name looked up on PATH (e.g. `claude`).
    pub binary: String,
    pub installed: bool,
    pub version: Option<String>,
    /// npm package that provides the binary (shown in the guide).
    pub package: String,
    /// Official install/setup docs (opened by the guide path).
    pub docs_url: String,
    /// Shell-capability command name for the one-click install (`install-<id>`).
    pub install_command_name: String,
    /// Exact args to pass — MUST match the capability allowlist entry, or the
    /// shell plugin rejects the spawn at runtime.
    pub install_args: Vec<String>,
}

/// The full status payload: every agent plus whether `npm` is available.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CliAgentsStatus {
    pub agents: Vec<CliAgentStatus>,
    /// `npm` on PATH — gates the one-click install (the guide always shows).
    pub npm_available: bool,
}

async fn build_status() -> CliAgentsStatus {
    let mut agents = Vec::new();
    for backend in cli_agent::all() {
        let binary = backend.binary();
        let (installed, version) = cli_agent::detect_cached(&binary).await;
        let id = backend.id().as_str().to_string();
        let install_package = backend.install_package();
        let (install_command_name, install_args, package) = if let Some(pkg) = install_package {
            (
                format!("install-{id}"),
                vec!["install".to_string(), "-g".to_string(), pkg.to_string()],
                pkg.to_string(),
            )
        } else {
            // No npm package — no one-click install, only guide path.
            (String::new(), Vec::new(), String::new())
        };
        agents.push(CliAgentStatus {
            install_command_name,
            install_args,
            id,
            binary,
            installed,
            version,
            package,
            docs_url: backend.docs_url().to_string(),
        });
    }
    // npm drives the one-click install; reuse the same cached probe as the agents.
    let (npm_available, _) = cli_agent::detect_cached("npm").await;
    CliAgentsStatus {
        agents,
        npm_available,
    }
}

/// Cached install status for all CLI agents (+ npm availability).
#[tauri::command]
pub async fn cli_agents_status() -> CliAgentsStatus {
    build_status().await
}

/// Clear the detection cache and re-probe — call after an in-app install, or
/// from a "Re-check" action, so a freshly-installed agent (or one that only
/// just finished a slow login-shell probe on macOS/Linux) shows as available
/// without an app restart.
#[tauri::command]
pub async fn cli_agents_redetect() -> CliAgentsStatus {
    cli_agent::clear_detect_cache();
    crate::platform::reset_cli_path_cache();
    build_status().await
}

#[cfg(test)]
mod tests;
