use super::*;
use serde_json::Value;

/// Security invariant: the static shell-capability allowlist must contain
/// EXACTLY the `npm install -g <package>` command for every registered agent
/// that has an npm package (matching `CliAgentBackend::install_package`).
/// Agents without an npm package (e.g. Cursor) have no allowlist entry.
#[test]
fn capability_allowlist_matches_the_registry() {
    let caps: Value =
        serde_json::from_str(include_str!("../../../capabilities/default.json")).unwrap();
    let allow = caps["permissions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p.get("identifier").and_then(|i| i.as_str()) == Some("shell:allow-execute"))
        .expect("shell:allow-execute permission present")
        .get("allow")
        .and_then(|a| a.as_array())
        .expect("allow scope present");

    let npm_agents: Vec<_> = cli_agent::all()
        .into_iter()
        .filter(|b| b.install_package().is_some())
        .collect();

    for backend in &npm_agents {
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
            vec!["install", "-g", backend.install_package().unwrap()],
            "{name} args must be the fixed global install for its package"
        );
    }

    // And nothing BEYOND the npm-installable agents is allowed to run.
    assert_eq!(
        allow.len(),
        npm_agents.len(),
        "allowlist has entries with no matching agent"
    );
}

#[tokio::test]
async fn status_lists_every_registered_agent_with_install_metadata() {
    let status = build_status().await;
    let ids: Vec<&str> = status.agents.iter().map(|a| a.id.as_str()).collect();
    for expected in [
        "claude-code",
        "codex",
        "gemini-cli",
        "antigravity",
        "opencode",
        "cursor",
        "qwen-code",
    ] {
        assert!(ids.contains(&expected), "{expected} missing from status");
    }
    for agent in &status.agents {
        if !agent.package.is_empty() {
            // Only npm-installable agents have a package and install command.
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
        } else {
            // Non-npm agents have no install command or package.
            assert!(agent.install_command_name.is_empty());
            assert!(agent.install_args.is_empty());
        }
        assert!(agent.docs_url.starts_with("https://"));
    }
}
