/// Extract the company name and role title from raw job ad text.
/// Uses lightweight heuristics — no LLM call, no I/O.
pub struct JobAdMeta {
    pub company: String,
    pub role: String,
}

pub fn extract(job_ad: &str) -> JobAdMeta {
    let company = extract_company(job_ad);
    let role = extract_role(job_ad);
    JobAdMeta { company, role }
}

fn extract_company(text: &str) -> String {
    // Priority 1: explicit labels
    for line in text.lines().take(40) {
        let lower = line.to_lowercase();
        for prefix in &["company:", "employer:", "organization:", "at "] {
            if let Some(rest) = lower.find(prefix).map(|i| &text[i + prefix.len()..]) {
                let candidate = rest.split(['|', '\n', ',', '(']).next().unwrap_or("").trim();
                if !candidate.is_empty() && candidate.len() < 80 {
                    return candidate.to_string();
                }
            }
        }
    }

    // Priority 2: "X is hiring" / "X is looking for" pattern
    let patterns = [" is hiring", " is looking for", " are hiring", " seeks a", " seeks an"];
    for pat in &patterns {
        if let Some(idx) = text.to_lowercase().find(pat) {
            // Walk backwards to find the start of the company name phrase
            let before = &text[..idx];
            let start = before.rfind('\n').map(|i| i + 1).unwrap_or(0);
            let candidate = before[start..].trim();
            if !candidate.is_empty() && candidate.len() < 80 {
                return candidate.to_string();
            }
        }
    }

    // Priority 3: "Join {Company}" / "About {Company}"
    for line in text.lines().take(60) {
        let lower = line.trim().to_lowercase();
        for prefix in &["join ", "about "] {
            if lower.starts_with(prefix) {
                let candidate = line.trim()[prefix.len()..].trim();
                if !candidate.is_empty() && candidate.len() < 80 {
                    return candidate.to_string();
                }
            }
        }
    }

    String::new()
}

fn extract_role(text: &str) -> String {
    // Priority 1: explicit labels on their own line or after a colon
    for line in text.lines().take(20) {
        let lower = line.to_lowercase();
        for prefix in &[
            "job title:",
            "position:",
            "role:",
            "title:",
            "we are hiring a",
            "we are looking for a",
            "we're hiring a",
            "we're looking for a",
        ] {
            if let Some(rest) = lower.find(prefix).map(|i| &text[i + prefix.len()..]) {
                let candidate = rest.split(['\n', '|', '(']).next().unwrap_or("").trim();
                if !candidate.is_empty() && candidate.len() < 100 {
                    return candidate.to_string();
                }
            }
        }
    }

    // Priority 2: first non-empty line (job ads usually start with the title)
    for line in text.lines().take(5) {
        let t = line.trim();
        if !t.is_empty() && t.len() < 100 {
            return t.to_string();
        }
    }

    String::new()
}
