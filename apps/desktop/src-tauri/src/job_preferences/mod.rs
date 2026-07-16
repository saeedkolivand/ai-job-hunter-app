use parking_lot::Mutex;
/// Job preferences store (SQLite-backed).
/// Stores user's job search preferences: location and tech stack.
use std::path::PathBuf;

use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};

use crate::data_store::DataStore;
use crate::db::{run_migrations, Migration};
use crate::error::AppResult;

// ── Types ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobPreferences {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub location: Option<String>,
    /// ISO 3166-1 alpha-2, captured alongside `location` from a picked geocode
    /// suggestion (mirrors `AutopilotTarget::country_code`) — lets a location
    /// seeded from here carry its real country instead of a scraper (the
    /// aggregator board) having to guess one. `#[serde(rename)]` on just this
    /// field (not `rename_all` on the whole struct) so the pre-existing
    /// `tech_stack` wire name is left untouched.
    #[serde(skip_serializing_if = "Option::is_none", rename = "countryCode")]
    pub country_code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tech_stack: Option<Vec<TechStackItem>>,
    /// User-supplied salary expectation (free text, e.g. "€75,000" or "80k
    /// DOE") — the backend-readable copy of the renderer's
    /// `usePreferencesStore.applicant.salaryExpectation` (Task #30). The
    /// renderer stays the source of truth (edited in Settings → Applicant
    /// details) and pushes it here on change + once on boot; the bridge's
    /// `answers.suggest` reads it to fill a synthetic salary-question row
    /// (`extension_bridge::answers_suggest`) that a renderer-only value could
    /// never satisfy. Clamped to [`MAX_SALARY_EXPECTATION_BYTES`] in [`set`](Self::set).
    #[serde(skip_serializing_if = "Option::is_none", rename = "salaryExpectation")]
    pub salary_expectation: Option<String>,
}

/// Byte cap on `salary_expectation` — this is free text, not a bounded enum,
/// so the store clamps it the same way `extension_bridge`'s own verb caps
/// clamp untrusted/renderer-supplied strings (`answers_suggest::clamp_bytes`
/// et al.), rather than trusting the renderer's own zod `.optional()` (no
/// length cap there) to always be the one write path.
const MAX_SALARY_EXPECTATION_BYTES: usize = 200;

/// Clamp `s` to at most `max` bytes, cutting on a UTF-8 char boundary — same
/// discipline as `extension_bridge::answers_suggest::clamp_bytes`.
fn clamp_bytes(mut s: String, max: usize) -> String {
    if s.len() <= max {
        return s;
    }
    let mut end = max;
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    s.truncate(end);
    s
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TechStackItem {
    pub name: String,
    pub category: String,
}

// ── JobPreferencesStore ─────────────────────────────────────────────────────────

pub struct JobPreferencesStore {
    conn: Mutex<Connection>,
}

impl JobPreferencesStore {
    const MIGRATIONS: &'static [Migration] = &[
        Migration {
            name: "create_job_preferences",
            up: |conn| {
                conn.execute_batch(
                    "CREATE TABLE IF NOT EXISTS job_preferences (
                    id INTEGER PRIMARY KEY CHECK (id = 1),
                    location TEXT,
                    remote TEXT,
                    seniority TEXT,
                    salary_min INTEGER,
                    salary_max INTEGER,
                    tech_stack TEXT
                );",
                )?;
                // Ensure the single settings row exists.
                conn.execute("INSERT OR IGNORE INTO job_preferences (id) VALUES (1)", [])?;
                Ok(())
            },
        },
        // Drop the dormant columns (remote, seniority, salary_min, salary_max) —
        // they were persisted but never written by any UI and only read by a dead
        // autopilot work-type seed. Use the SQLite-safe table-recreate (works on
        // every bundled SQLite regardless of `ALTER TABLE ... DROP COLUMN` support)
        // so location + tech_stack survive the column removal.
        Migration {
            name: "drop_unused_job_preferences_columns",
            up: |conn| {
                conn.execute_batch(
                    "CREATE TABLE job_preferences_new (
                        id INTEGER PRIMARY KEY CHECK (id = 1),
                        location TEXT,
                        tech_stack TEXT
                    );
                    INSERT INTO job_preferences_new (id, location, tech_stack)
                        SELECT id, location, tech_stack FROM job_preferences;
                    DROP TABLE IF EXISTS job_preferences;
                    ALTER TABLE job_preferences_new RENAME TO job_preferences;
                    INSERT OR IGNORE INTO job_preferences (id) VALUES (1);",
                )?;
                Ok(())
            },
        },
        // Add the country code captured alongside `location` from a geocode pick
        // (autopilot aggregator zero-jobs fix) — a plain `ADD COLUMN` is safe here
        // (unlike the DROP COLUMN above, every bundled SQLite supports it).
        Migration {
            name: "add_job_preferences_country_code",
            up: |conn| {
                conn.execute_batch("ALTER TABLE job_preferences ADD COLUMN country_code TEXT;")?;
                Ok(())
            },
        },
        // Backend-readable salary expectation (Task #30) — a plain `ADD COLUMN`,
        // same safe shape as the country-code migration above.
        Migration {
            name: "add_job_preferences_salary_expectation",
            up: |conn| {
                conn.execute_batch(
                    "ALTER TABLE job_preferences ADD COLUMN salary_expectation TEXT;",
                )?;
                Ok(())
            },
        },
    ];

    pub fn open(data_dir: &PathBuf) -> AppResult<Self> {
        std::fs::create_dir_all(data_dir)?;
        let path = data_dir.join("job_preferences.db");
        let mut conn = crate::db::open(&path)?;
        run_migrations(&mut conn, Self::MIGRATIONS)?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    pub fn get(&self) -> JobPreferences {
        let conn = self.conn.lock();
        conn.query_row(
            "SELECT location, tech_stack, country_code, salary_expectation
             FROM job_preferences WHERE id = 1",
            [],
            |row| {
                let tech_stack_json: Option<String> = row.get(1)?;
                let tech_stack = tech_stack_json.and_then(|s| serde_json::from_str(&s).ok());
                Ok(JobPreferences {
                    location: row.get(0)?,
                    country_code: row.get(2)?,
                    tech_stack,
                    salary_expectation: row.get(3)?,
                })
            },
        )
        .unwrap_or(JobPreferences {
            location: None,
            country_code: None,
            tech_stack: None,
            salary_expectation: None,
        })
    }

    /// Reset all job preferences to empty (factory reset).
    pub fn clear(&self) -> AppResult<()> {
        let conn = self.conn.lock();
        conn.execute(
            "UPDATE job_preferences SET location = NULL, tech_stack = NULL, country_code = NULL, salary_expectation = NULL WHERE id = 1",
            [],
        )
        .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn set(&self, prefs: &JobPreferences) -> AppResult<()> {
        let conn = self.conn.lock();
        let tech_stack_json = prefs
            .tech_stack
            .as_ref()
            .and_then(|ts| serde_json::to_string(ts).ok());
        // Untrusted free text (not a bounded enum) — clamp server-side rather
        // than trusting the renderer's own validation to be the only write path.
        let salary_expectation = prefs
            .salary_expectation
            .clone()
            .map(|s| clamp_bytes(s, MAX_SALARY_EXPECTATION_BYTES));

        conn.execute(
            "UPDATE job_preferences
             SET location = ?1, tech_stack = ?2, country_code = ?3, salary_expectation = ?4
             WHERE id = 1",
            params![
                prefs.location,
                tech_stack_json,
                prefs.country_code,
                salary_expectation
            ],
        )
        .map_err(|e| e.to_string())?;
        Ok(())
    }
}

impl DataStore for JobPreferencesStore {
    fn key(&self) -> &'static str {
        "jobPreferences"
    }

    fn export(&self) -> serde_json::Value {
        serde_json::to_value(self.get()).unwrap_or_else(|_| serde_json::json!({}))
    }

    fn import(&self, data: &serde_json::Value) -> AppResult<usize> {
        // Single settings row; treat null/missing as "nothing to restore".
        if data.is_null() {
            return Ok(0);
        }
        let prefs: JobPreferences =
            serde_json::from_value(data.clone()).map_err(|e| e.to_string())?;
        self.set(&prefs)?;
        Ok(1)
    }
}

#[cfg(test)]
mod test;
