use super::*;

#[test]
fn test_system_get_version() {
    let version = system_get_version();
    assert!(!version.is_empty());
    // Version should be semver-like (x.y.z)
    assert!(version.contains('.'));
}

#[test]
fn test_system_get_platform() {
    let platform = system_get_platform();
    assert!(platform["platform"].is_string());
    assert!(platform["arch"].is_string());
    assert_eq!(platform["shell"], "tauri");
}

#[test]
fn test_locale_file_path() {
    // This test would need a mock AppHandle in practice
    // For now, we'll skip the full integration test
}

#[test]
fn test_gpu_info_empty() {
    #[cfg(not(windows))]
    {
        let gpu_info = get_gpu_info();
        // On non-Windows, this may return empty or actual GPU info
        // Just verify it returns a vector without panicking
        let _ = gpu_info;
    }
}

#[test]
fn active_provider_summary_reports_the_configured_provider_and_model() {
    let active = crate::ai_config::ActiveAiConfig {
        active_provider: Some("openai".to_string()),
        model: Some("gpt-5".to_string()),
        ..Default::default()
    };
    let out = active_provider_summary(Some(active));
    assert_eq!(out["provider"], "openai");
    assert_eq!(out["model"], "gpt-5");
}

#[test]
fn active_provider_summary_degrades_to_nulls_when_the_store_is_unavailable() {
    let out = active_provider_summary(None);
    assert!(out["provider"].is_null());
    assert!(out["model"].is_null());
}

#[test]
fn active_provider_summary_reports_nulls_when_unseeded() {
    // A real store that has never had a provider selected reports `None`
    // for both fields (see `ActiveAiConfig`'s doc comment) — must not be
    // confused with the store-unavailable case above.
    let out = active_provider_summary(Some(crate::ai_config::ActiveAiConfig::default()));
    assert!(out["provider"].is_null());
    assert!(out["model"].is_null());
}

#[test]
fn test_system_check_browser() {
    // Test that the function doesn't panic and returns valid JSON
    let result = system_check_browser();
    // Verify the result has the expected structure
    assert!(result.is_object());
    assert!(result.get("detected").is_some());
    assert!(result.get("path").is_some());
}
