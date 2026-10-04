//! Best-effort GPU enumeration for `system_get_metrics` (WMI on Windows, sysfs/lspci on
//! Linux, `system_profiler` on macOS). Every probe degrades to an empty list.

use serde_json::{json, Value};

#[cfg(windows)]
pub(super) fn get_gpu_info() -> Vec<Value> {
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
pub(super) fn get_gpu_info() -> Vec<Value> {
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
