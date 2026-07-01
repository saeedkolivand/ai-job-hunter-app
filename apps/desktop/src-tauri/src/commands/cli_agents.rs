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
        agents.push(CliAgentStatus {
            install_command_name: format!("install-{id}"),
            install_args: vec![
                "install".to_string(),
                "-g".to_string(),
                backend.install_package().to_string(),
            ],
            id,
            binary,
            installed,
            version,
            package: backend.install_package().to_string(),
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

/// Clear the detection cache and re-probe — call after an in-app install so a
/// freshly-installed agent shows as available immediately.
#[tauri::command]
pub async fn cli_agents_redetect() -> CliAgentsStatus {
    cli_agent::clear_detect_cache();
    build_status().await
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    /// Security invariant: the static shell-capability allowlist must contain
    /// EXACTLY the `npm install -g <package>` command for every registered agent
    /// (matching `CliAgentBackend::install_package`). If the registry and the
    /// allowlist drift, one-click install silently breaks (or, worse, an
    /// unintended command becomes runnable) — this catches both.
    #[test]
    fn capability_allowlist_matches_the_registry() {
        let caps: Value =
            serde_json::from_str(include_str!("../../capabilities/default.json")).unwrap();
        let allow = caps["permissions"]
            .as_array()
            .unwrap()
            .iter()
            .find(|p| p.get("identifier").and_then(|i| i.as_str()) == Some("shell:allow-execute"))
            .expect("shell:allow-execute permission present")
            .get("allow")
            .and_then(|a| a.as_array())
            .expect("allow scope present");

        for backend in cli_agent::all() {
            let name = format!("install-{}", backend.id().as_str());
            let entry = allow
                .iter()
                .find(|e| e.get("name").and_then(|n| n.as_str()) == Some(name.as_str()))
                .unwrap_or_else(|| panic!("allowlist missing entry {name}"));
            assert_eq!(entry["cmd"].as_str(), Some("npm"), "{name} must run npm");
            let args: Vec<&str> = entry["args"]
                .as_array()
                .unwrap()
                .iter()
                .filter_map(|a| a.as_str())
                .collect();
            assert_eq!(
                args,
                vec!["install", "-g", backend.install_package()],
                "{name} args must be the fixed global install for its package"
            );
        }

        // And nothing BEYOND the registered agents is allowed to run.
        assert_eq!(
            allow.len(),
            cli_agent::all().len(),
            "allowlist has entries with no matching agent"
        );
    }

    #[tokio::test]
    async fn status_lists_every_registered_agent_with_install_metadata() {
        let status = build_status().await;
        let ids: Vec<&str> = status.agents.iter().map(|a| a.id.as_str()).collect();
        for expected in ["claude-code", "codex", "gemini-cli"] {
            assert!(ids.contains(&expected), "{expected} missing from status");
        }
        for agent in &status.agents {
            // The one-click command is always `npm install -g <package>`, fixed.
            assert_eq!(agent.install_command_name, format!("install-{}", agent.id));
            assert_eq!(
                agent.install_args,
                vec![
                    "install".to_string(),
                    "-g".to_string(),
                    agent.package.clone()
                ]
            );
            assert!(agent.package.starts_with('@'), "scoped package expected");
            assert!(agent.docs_url.starts_with("https://"));
        }
    }
}
