use serde_json::{json, Map, Value};
use tauri::{AppHandle, Manager};
use crate::commands::ai_provider::cli_agent;
use crate::error::{AppError, AppResult};
use crate::scraping::ScraperEngine;

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
        "data": { "ready": true, "sqlite": true, "vector": true },
        "workers": { "active": 0, "idle": 1, "max": 1 }
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

#[tauri::command]
pub async fn system_set_performance_mode(app: AppHandle, mode: String) -> Value {
    // Update the engine's performance mode
    let engine = app.state::<std::sync::Arc<ScraperEngine>>();
    engine.set_performance_mode(&mode);
    json!(null)
}

#[cfg(windows)]
fn get_gpu_info() -> Vec<Value> {
    use wmi::WMIConnection;
    use serde::Deserialize;

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
                    let gpu_name = if let Ok(uevent_content) = fs::read_to_string(device_path.join("uevent")) {
                        let mut name = None;
                        for line in uevent_content.lines() {
                            if line.starts_with("PRODUCT=") {
                                name = Some(line.strip_prefix("PRODUCT=").unwrap_or("Unknown GPU").to_string());
                            } else if line.starts_with("PCI_ID=") {
                                name = Some(line.strip_prefix("PCI_ID=").unwrap_or("Unknown GPU").to_string());
                            }
                        }
                        name
                    } else {
                        None
                    };
                    
                    let name = gpu_name.unwrap_or_else(|| {
                        // Fallback: try reading from modalias
                        if let Ok(modalias) = fs::read_to_string(device_path.join("modalias")) {
                            modalias.trim().split(':').next().unwrap_or("Unknown GPU").to_string()
                        } else {
                            "Unknown GPU".to_string()
                        }
                    });
                    
                    // Try to get VRAM from sysfs (newer kernels)
                    let vram_total = if let Ok(vram_str) = fs::read_to_string(device_path.join("mem_info_vram_total")) {
                        vram_str.trim().parse::<u64>().unwrap_or(0) / 1024 // Convert KB to MB
                    } else if let Ok(vram_str) = fs::read_to_string(device_path.join("mem_total_vram")) {
                        vram_str.trim().parse::<u64>().unwrap_or(0) / 1024
                    } else {
                        // Try lspci as fallback
                        let vram_from_lspci = if let Ok(output) = std::process::Command::new("lspci")
                            .args(["-v"])
                            .output()
                        {
                            let lspci_output = String::from_utf8_lossy(&output.stdout);
                            let mut vram = None;
                            for line in lspci_output.lines() {
                                if line.contains("prefetchable") && line.contains("size=") {
                                    // Parse VRAM from lspci output (e.g., "size=8192M")
                                    if let Some(size_str) = line.split("size=").nth(1) {
                                        let size = size_str.trim_end_matches('M').trim_end_matches('G');
                                        if let Ok(size_mb) = size.parse::<u64>() {
                                            let multiplier = if line.contains('G') { 1024 } else { 1 };
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
                    if let Some(displays) = root.get("SPDisplaysDataType").and_then(|v| v.as_array()) {
                        for display in displays {
                            if let Some(name) = display.get("smbio").and_then(|v| v.as_str()) {
                                if let Some(vram_str) = display.get("vram_mb").and_then(|v| v.as_str()) {
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
    let cpu_percent = sys.cpus().iter().map(|c| c.cpu_usage()).sum::<f32>()
        / sys.cpus().len().max(1) as f32;
    let cpu_count = sys.cpus().len();
    let cpu_brand = sys.cpus().first().map(|c| c.brand().to_string()).unwrap_or_default();
    let cpu_name = sys.cpus().first().map(|c| c.name().to_string()).unwrap_or_default();
    
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
const PROTOCOL_VERSION: &str = "1.0.0";

#[tauri::command]
pub fn system_get_protocol_version() -> String {
    PROTOCOL_VERSION.to_string()
}

#[cfg(test)]
mod test;
