use crate::commands::ai_provider::cli_agent;
use crate::documents::DocumentStore;
use crate::error::{AppError, AppResult};
use crate::scraping::ScraperEngine;
use serde::Deserialize;
use serde_json::{json, Map, Value};
use tauri::{AppHandle, Manager};

#[tauri::command]
pub async fn system_health(app: AppHandle) -> Value {
    let engine = app.state::<std::sync::Arc<ScraperEngine>>();
    let scraper_health = engine.health();

    // Local (Ollama) availability + running model, via the Ollama provider module.
    let (ai_ready, ai_model) = crate::commands::ai_provider::ollama::reachable_model().await;

    // CLI agents (Claude Code, …): "detected" = their binary is installed. Looped
    // from the registry so new agents appear here automatically.
    let mut cli_agents = Map::new();
    for backend in cli_agent::all() {
        let (detected, version) = cli_agent::detect_cached(&backend.binary()).await;
        cli_agents.insert(
            backend.id().as_str().to_string(),
            json!({ "detected": detected, "version": version }),
        );
    }

    json!({
        "status": "ok",
        "shell": "tauri",
        "scraper": { "mode": scraper_health.mode, "ready": scraper_health.ready, "scrapers": scraper_health.scrapers },
        "ai": { "ready": ai_ready, "model": ai_model },
        "cliAgents": Value::Object(cli_agents),
        "data": { "ready": true, "sqlite": true, "vector": true }
    })
}

#[tauri::command]
pub fn system_get_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

#[tauri::command]
pub fn system_get_locale(app: AppHandle) -> Value {
    let locale = read_locale_file(&app);
    json!(locale)
}

#[tauri::command]
pub fn system_set_locale(app: AppHandle, locale: String) -> Value {
    write_locale_file(&app, &locale);
    json!(null)
}

fn locale_file_path(app: &AppHandle) -> std::path::PathBuf {
    app.path()
        .app_data_dir()
        .unwrap_or_else(|_| std::path::PathBuf::from("."))
        .join("locale.json")
}

fn read_locale_file(app: &AppHandle) -> String {
    std::fs::read_to_string(locale_file_path(app))
        .ok()
        .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
        .and_then(|v| v.get("locale").and_then(|l| l.as_str()).map(String::from))
        .unwrap_or_else(|| "en".to_string())
}

fn write_locale_file(app: &AppHandle, locale: &str) {
    let path = locale_file_path(app);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).ok();
    }
    let content = serde_json::json!({ "locale": locale });
    std::fs::write(path, serde_json::to_string(&content).unwrap_or_default()).ok();
}

#[tauri::command]
pub fn system_check_browser() -> Value {
    let detected = crate::platform::detect_system_chrome();
    json!({
        "detected": detected.is_some(),
        "path": detected.map(|p| p.to_string_lossy().to_string())
    })
}

/// Best-effort read of the OS accent color as `#rrggbb`.
///
/// `supported` is true only on platforms we can read (Windows, macOS); Linux and
/// any read failure report a null color so the renderer silently keeps the
/// Default accent — the UI never shows an "unsupported" error. The `System`
/// accent option is hidden when `supported` is false.
#[tauri::command]
pub fn system_accent_color() -> Value {
    json!({
        "supported": cfg!(any(windows, target_os = "macos")),
        "color": read_accent_hex(),
    })
}

#[cfg(windows)]
fn read_accent_hex() -> Option<String> {
    // UISettings returns the true accent (honouring "Automatic accent color"),
    // unlike DwmGetColorizationColor which yields the colorization tint.
    use windows::UI::ViewManagement::{UIColorType, UISettings};
    let settings = UISettings::new().ok()?;
    let c = settings.GetColorValue(UIColorType::Accent).ok()?;
    Some(format!("#{:02x}{:02x}{:02x}", c.R, c.G, c.B))
}

#[cfg(target_os = "macos")]
fn read_accent_hex() -> Option<String> {
    // macOS accent is a FIXED palette (not an arbitrary color): `defaults read -g
    // AppleAccentColor` is an integer; absent = the multicolor/blue default. The
    // integer→hex map is therefore exact for macOS's named accents — and dep-free.
    let out = std::process::Command::new("defaults")
        .args(["read", "-g", "AppleAccentColor"])
        .output()
        .ok()?;
    let hex = match String::from_utf8_lossy(&out.stdout).trim() {
        "0" => "#ff5257",         // red
        "1" => "#f8821a",         // orange
        "2" => "#ffc402",         // yellow
        "3" => "#62ba46",         // green
        "5" => "#a550a7",         // purple
        "6" => "#f74f9e",         // pink
        "-1" | "-2" => "#8c8c8c", // graphite
        _ => "#007aff",           // 4 / absent / multicolor → blue
    };
    Some(hex.to_string())
}

#[cfg(not(any(windows, target_os = "macos")))]
fn read_accent_hex() -> Option<String> {
    None
}

#[tauri::command]
pub fn system_get_platform() -> Value {
    json!({
        "platform": std::env::consts::OS,
        "arch": std::env::consts::ARCH,
        "shell": "tauri"
    })
}

#[tauri::command]
pub fn system_open_devtools(app: AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        window.open_devtools();
    }
}

#[tauri::command]
pub async fn system_open_external(app: AppHandle, url: String) -> AppResult<()> {
    use tauri_plugin_opener::OpenerExt;
    app.opener()
        .open_url(&url, None::<&str>)
        .map_err(|e| AppError::Message(e.to_string()))
}

/// Backend tuning payload from the renderer's performance-mode picker. camelCase
/// to match the JS the renderer already sends via `system_set_performance_mode`.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PerformanceBackendConfig {
    /// Informational label only (e.g. "battery"/"balanced"/"performance"). Backend
    /// behavior is driven by the numeric knobs below, never by branching on `mode`.
    pub mode: String,
    pub concurrency: u32,
    pub keep_alive_secs: u64,
    pub cache_ttl_secs: Option<i64>,
    pub cache_max_rows: Option<i64>,
}

#[tauri::command]
pub async fn system_set_performance_mode(
    app: AppHandle,
    config: PerformanceBackendConfig,
) -> Value {
    // Concurrency → scraper engine semaphore. Clamp at the trust boundary so a
    // buggy renderer can't request a huge semaphore (upper) or a zero one (lower,
    // also guarded by set_concurrency's .max(1)).
    let concurrency = (config.concurrency as usize).clamp(1, 16);
    app.state::<std::sync::Arc<ScraperEngine>>()
        .set_concurrency(concurrency);

    // Keep-alive + cache knobs → live performance config (read by the Ollama
    // adapter and the cache eviction sites). Set BEFORE the prune below so the
    // one-shot prune sees the new bounds. Coerce the cache bounds non-negative: a
    // negative SQLite LIMIT means "no limit" (silently disables the cap) and a
    // negative ttl expires everything — both footguns from a buggy renderer.
    let cache_max_rows = config.cache_max_rows.map(|x| x.max(0));
    let cache_ttl_secs = config.cache_ttl_secs.map(|x| x.max(0));
    crate::performance::set(crate::performance::PerformanceConfig {
        keep_alive_secs: config.keep_alive_secs,
        cache_ttl_secs,
        cache_max_rows,
    });

    // One-shot prune so tightening a tier reclaims immediately. DocumentStore
    // is managed only if its open() succeeded at boot (treated as non-fatal in
    // lib.rs), so use the fallible accessor to avoid panicking on a degraded
    // startup.
    if let Some(store) = app.try_state::<DocumentStore>() {
        store.prune_caches(cache_ttl_secs, cache_max_rows);
    } else {
        tracing::warn!(
            "system_set_performance_mode: DocumentStore unavailable; skipping one-shot cache prune"
        );
    }

    json!(null)
}

/// Whether the app is registered to launch at login. Off by default; a read
/// error (e.g. the OS autostart entry is missing) reports `false`.
#[tauri::command]
pub fn system_get_launch_at_login(app: AppHandle) -> bool {
    use tauri_plugin_autostart::ManagerExt;
    app.autolaunch().is_enabled().unwrap_or(false)
}

/// Enable or disable launch-at-login. Returns the resulting state so the UI
/// reflects what the OS actually applied rather than the requested value.
#[tauri::command]
pub fn system_set_launch_at_login(app: AppHandle, enabled: bool) -> AppResult<bool> {
    use tauri_plugin_autostart::ManagerExt;
    let manager = app.autolaunch();
    if enabled {
        manager.enable()
    } else {
        manager.disable()
    }
    .map_err(|e| AppError::Message(e.to_string()))?;
    Ok(manager.is_enabled().unwrap_or(enabled))
}

/// Set whether closing the window hides the app to the tray (true) or quits it
/// (false). The renderer's preferences store owns this value and pushes it here
/// on boot and on every change; the window-close handler in `lib.rs` reads the
/// managed [`crate::CloseToTray`] flag. No getter — the renderer is the source
/// of truth, so there is nothing to read back.
#[tauri::command]
pub fn system_set_close_to_tray(app: AppHandle, enabled: bool) {
    *app.state::<crate::CloseToTray>().0.lock() = enabled;
}

#[cfg(windows)]
fn get_gpu_info() -> Vec<Value> {
    use serde::Deserialize;
    use wmi::WMIConnection;

    #[derive(Deserialize, Debug)]
    struct VideoController {
        name: String,
        adapter_ram: Option<u64>,
    }

    let mut gpu_info = Vec::new();

    if let Ok(wmi_con) = WMIConnection::new() {
        if let Ok(results) = wmi_con.query::<VideoController>() {
            for gpu in results {
                let vram_total = gpu.adapter_ram.unwrap_or(0) / (1024 * 1024); // Convert bytes to MB
                gpu_info.push(json!({
                    "name": gpu.name,
                    "vramTotal": vram_total,
                    "vramUsed": 0, // WMI doesn't provide current usage
                    "vramFree": vram_total,
                }));
            }
        }
    }

    gpu_info
}

#[cfg(not(windows))]
fn get_gpu_info() -> Vec<Value> {
    let mut gpu_info = Vec::new();

    #[cfg(target_os = "linux")]
    {
        // Linux: Read GPU info from sysfs and lspci
        use std::fs;

        if let Ok(entries) = fs::read_dir("/sys/class/drm") {
            for entry in entries.flatten() {
                let path = entry.path();
                let device_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");

                // Only look at card devices (card0, card1, etc.)
                if device_name.starts_with("card") && !device_name.contains('-') {
                    let device_path = path.join("device");

                    // Try to get GPU name from uevent
                    let gpu_name = if let Ok(uevent_content) =
                        fs::read_to_string(device_path.join("uevent"))
                    {
                        let mut name = None;
                        for line in uevent_content.lines() {
                            if line.starts_with("PRODUCT=") {
                                name = Some(
                                    line.strip_prefix("PRODUCT=")
                                        .unwrap_or("Unknown GPU")
                                        .to_string(),
                                );
                            } else if line.starts_with("PCI_ID=") {
                                name = Some(
                                    line.strip_prefix("PCI_ID=")
                                        .unwrap_or("Unknown GPU")
                                        .to_string(),
                                );
                            }
                        }
                        name
                    } else {
                        None
                    };

                    let name = gpu_name.unwrap_or_else(|| {
                        // Fallback: try reading from modalias
                        if let Ok(modalias) = fs::read_to_string(device_path.join("modalias")) {
                            modalias
                                .trim()
                                .split(':')
                                .next()
                                .unwrap_or("Unknown GPU")
                                .to_string()
                        } else {
                            "Unknown GPU".to_string()
                        }
                    });

                    // Try to get VRAM from sysfs (newer kernels)
                    let vram_total = if let Ok(vram_str) =
                        fs::read_to_string(device_path.join("mem_info_vram_total"))
                    {
                        vram_str.trim().parse::<u64>().unwrap_or(0) / 1024 // Convert KB to MB
                    } else if let Ok(vram_str) =
                        fs::read_to_string(device_path.join("mem_total_vram"))
                    {
                        vram_str.trim().parse::<u64>().unwrap_or(0) / 1024
                    } else {
                        // Try lspci as fallback
                        let vram_from_lspci = if let Ok(output) =
                            std::process::Command::new("lspci").args(["-v"]).output()
                        {
                            let lspci_output = String::from_utf8_lossy(&output.stdout);
                            let mut vram = None;
                            for line in lspci_output.lines() {
                                if line.contains("prefetchable") && line.contains("size=") {
                                    // Parse VRAM from lspci output (e.g., "size=8192M")
                                    if let Some(size_str) = line.split("size=").nth(1) {
                                        let size =
                                            size_str.trim_end_matches('M').trim_end_matches('G');
                                        if let Ok(size_mb) = size.parse::<u64>() {
                                            let multiplier =
                                                if line.contains('G') { 1024 } else { 1 };
                                            vram = Some(size_mb * multiplier);
                                        }
                                    }
                                }
                            }
                            vram
                        } else {
                            None
                        };
                        vram_from_lspci.unwrap_or(0)
                    };

                    gpu_info.push(json!({
                        "name": name,
                        "vramTotal": vram_total,
                        "vramUsed": 0,
                        "vramFree": vram_total,
                    }));
                }
            }
        }
    }

    #[cfg(target_os = "macos")]
    {
        // macOS: Use system_profiler command to get GPU info
        use std::process::Command;

        if let Ok(output) = Command::new("system_profiler")
            .args(["SPDisplaysDataType", "-json"])
            .output()
        {
            if let Ok(json_str) = String::from_utf8(output.stdout) {
                if let Ok(root) = serde_json::from_str::<serde_json::Value>(&json_str) {
                    if let Some(displays) =
                        root.get("SPDisplaysDataType").and_then(|v| v.as_array())
                    {
                        for display in displays {
                            if let Some(name) = display.get("smbio").and_then(|v| v.as_str()) {
                                if let Some(vram_str) =
                                    display.get("vram_mb").and_then(|v| v.as_str())
                                {
                                    if let Ok(vram_mb) = vram_str.parse::<u64>() {
                                        gpu_info.push(json!({
                                            "name": name,
                                            "vramTotal": vram_mb,
                                            "vramUsed": 0,
                                            "vramFree": vram_mb,
                                        }));
                                    }
                                } else {
                                    gpu_info.push(json!({
                                        "name": name,
                                        "vramTotal": 0,
                                        "vramUsed": 0,
                                        "vramFree": 0,
                                    }));
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    gpu_info
}

#[tauri::command]
pub fn system_get_metrics() -> Value {
    use sysinfo::System;
    let mut sys = System::new_all();
    sys.refresh_all();

    let total_mem = sys.total_memory();
    let used_mem = sys.used_memory();
    let uptime = System::uptime();
    let cpu_percent =
        sys.cpus().iter().map(|c| c.cpu_usage()).sum::<f32>() / sys.cpus().len().max(1) as f32;
    let cpu_count = sys.cpus().len();
    let cpu_brand = sys
        .cpus()
        .first()
        .map(|c| c.brand().to_string())
        .unwrap_or_default();
    let cpu_name = sys
        .cpus()
        .first()
        .map(|c| c.name().to_string())
        .unwrap_or_default();

    let gpu_info = get_gpu_info();

    json!({
        "shell": "tauri",
        "uptime": uptime,
        "memoryMb": used_mem / 1024 / 1024,
        "totalMemoryMb": total_mem / 1024 / 1024,
        "cpuPercent": (cpu_percent * 10.0).round() / 10.0,
        "cpuCount": cpu_count,
        "cpuBrand": cpu_brand,
        "cpuName": cpu_name,
        "gpus": gpu_info
    })
}

/// IPC contract version. Must stay in sync with `PROTOCOL_VERSION` in
/// `packages/shared/src/ipc/contracts/index.ts`. Bump both together on any
/// breaking change to a command signature or event payload shape.
const PROTOCOL_VERSION: &str = "1.1.0";

#[tauri::command]
pub fn system_get_protocol_version() -> String {
    PROTOCOL_VERSION.to_string()
}

#[cfg(test)]
mod test;
