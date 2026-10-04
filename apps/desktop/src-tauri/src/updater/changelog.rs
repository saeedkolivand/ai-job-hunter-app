use serde_json::{json, Value};

/// Most recent releases to surface in the in-app changelog.
const CHANGELOG_LIMIT: usize = 15;

/// The repo's own `CHANGELOG.md`, bundled into the binary at compile time — the
/// changelog now works fully offline and makes no GitHub request (removes an
/// egress path the old per-release API fetch had). `include_str!` makes rustc
/// track the file for rebuilds like any other source dependency, no `build.rs`
/// needed. `@semantic-release/changelog` (`release.config.mjs`) regenerates this
/// file as part of the release commit, before the Tauri build step packages this
/// binary — see the release workflow for the ordering.
pub(super) const CHANGELOG_MD: &str = include_str!("../../../../../CHANGELOG.md");

/// One version section parsed out of [`CHANGELOG_MD`].
struct ChangelogEntry {
    version: String,
    date: Option<String>,
    body: String,
}

/// Splits a changelog document into per-version entries, in file order (the
/// generator writes newest-first). `@semantic-release/changelog` writes headings
/// shaped `## [x.y.z](compare-url) (YYYY-MM-DD)`; any other line — including a
/// malformed or empty document — simply yields no entry for that line rather
/// than erroring, so a reformatted/corrupted changelog degrades to fewer
/// releases instead of panicking.
fn parse_changelog(raw: &str) -> Vec<ChangelogEntry> {
    let mut entries = Vec::new();
    let mut current: Option<(String, Option<String>, usize)> = None;
    let mut offset = 0usize;

    for line in raw.split_inclusive('\n') {
        if let Some((version, date)) = parse_heading(line) {
            if let Some((v, d, body_start)) = current.take() {
                entries.push(ChangelogEntry {
                    version: v,
                    date: d,
                    body: raw[body_start..offset].trim().to_string(),
                });
            }
            current = Some((version, date, offset + line.len()));
        }
        offset += line.len();
    }
    if let Some((v, d, body_start)) = current {
        entries.push(ChangelogEntry {
            version: v,
            date: d,
            body: raw[body_start..].trim().to_string(),
        });
    }
    entries
}

/// Parses one `## [x.y.z](...) (YYYY-MM-DD)` version heading line. `None` for any
/// other line (subsection headings like `### Features`, prose, blank lines). This
/// also deliberately excludes `@semantic-release/changelog`'s first-ever-release
/// heading shape `## x.y.z` (no `[...]` link, since there's no prior tag to
/// compare against) — harmless in practice, since `CHANGELOG_LIMIT` means the
/// capped, newest-first list never reaches that far back.
fn parse_heading(line: &str) -> Option<(String, Option<String>)> {
    let rest = line.trim_end().strip_prefix("## [")?;
    let (version, rest) = rest.split_once(']')?;
    if !version.starts_with(|c: char| c.is_ascii_digit()) {
        return None;
    }
    // Shape is `(compare-url) (YYYY-MM-DD)` — the date is the last `(...)`.
    let date = rest
        .rsplit_once('(')
        .and_then(|(_, tail)| tail.strip_suffix(')'))
        .filter(|d| d.len() == 10)
        .map(str::to_string);
    Some((version.to_string(), date))
}

/// Builds the `updater_changelog` reply from raw changelog text — split out from
/// the command so tests can exercise the malformed/empty path without needing a
/// separate on-disk fixture.
pub(super) fn changelog_response(raw: &str) -> Value {
    let entries = parse_changelog(raw);
    if entries.is_empty() {
        return json!({ "error": "Changelog unavailable (bundled CHANGELOG.md has no releases)." });
    }

    let items: Vec<Value> = entries
        .into_iter()
        .take(CHANGELOG_LIMIT)
        .map(|e| {
            let url = format!(
                "https://github.com/saeedkolivand/ai-job-hunter-app/releases/tag/v{}",
                e.version
            );
            json!({
                "version": e.version,
                "name": null,
                "body": e.body,
                "publishedAt": e.date,
                "url": url,
                "prerelease": e.version.contains('-'),
            })
        })
        .collect();
    json!({ "releases": items })
}

#[cfg(test)]
mod tests;
