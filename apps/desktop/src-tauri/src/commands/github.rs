use serde_json::{json, Value};

/// Fetch a user's public GitHub repos for the resume-builder import flow.
///
/// `input` is a bare username or a `github.com/<user>` URL; the backend extracts
/// + validates the username and constructs the api.github.com URL itself (the
/// renderer never supplies the request URL). Returns `{ repos: [...] }` on Ok or
/// `{ error: "..." }` on failure — mirrors `profile_import_from_url`'s envelope.
#[tauri::command]
pub async fn github_import_repos(input: String) -> Value {
    match crate::profile_import::github::fetch_repos(&input).await {
        Ok(repos) => json!({ "repos": repos }),
        Err(e) => json!({ "error": e.to_string() }),
    }
}
