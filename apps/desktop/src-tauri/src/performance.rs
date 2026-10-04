//! Live performance-mode tuning values (concurrency lives on ScraperEngine; this
//! holds the keep-alive + cache knobs). Single process-global source of truth so
//! the Ollama embed builder — a free fn with no AppHandle — and the cache call
//! sites read the same values. Updated by `system_set_performance_mode`.
use arc_swap::ArcSwap;
use std::sync::Arc;
use std::sync::OnceLock;

#[derive(Debug, Clone)]
pub struct PerformanceConfig {
    pub keep_alive_secs: u64,
    pub cache_ttl_secs: Option<i64>,
    pub cache_max_rows: Option<i64>,
}

impl Default for PerformanceConfig {
    /// Balanced tier (the renderer's default mode): 300s keep-alive, 7-day TTL, 2000-row cap.
    fn default() -> Self {
        Self {
            keep_alive_secs: 300,
            cache_ttl_secs: Some(604_800),
            cache_max_rows: Some(2000),
        }
    }
}

static CONFIG: OnceLock<ArcSwap<PerformanceConfig>> = OnceLock::new();

fn cell() -> &'static ArcSwap<PerformanceConfig> {
    CONFIG.get_or_init(|| ArcSwap::from_pointee(PerformanceConfig::default()))
}

/// Live snapshot of the current performance config.
pub fn current() -> Arc<PerformanceConfig> {
    cell().load_full()
}

/// Replace the live performance config (called by the command on mode apply).
pub fn set(cfg: PerformanceConfig) {
    cell().store(Arc::new(cfg));
}

/// Ollama `keep_alive` wire value from the live config: 0 → "0" (unload now),
/// else "{secs}s". Only the Ollama adapter consumes this.
pub fn ollama_keep_alive() -> String {
    keep_alive_value(current().keep_alive_secs)
}

/// Pure mapping (unit-testable without the global).
pub(crate) fn keep_alive_value(secs: u64) -> String {
    if secs == 0 {
        "0".to_string()
    } else {
        format!("{secs}s")
    }
}

#[cfg(test)]
mod tests;
