use regex::Regex;
use std::sync::OnceLock;

/// Fix OCR artifacts. Only ever called on OCR output.
pub fn fix(text: &str) -> String {
    // Emails and phones must run before digit collapsing: fix_spaced_digits
    // would join "9 7" into "97" first, breaking the email/phone patterns.
    let text = fix_spaced_emails(text);
    let text = fix_spaced_phone_numbers(&text);
    let text = fix_spaced_digits(&text);
    let text = collapse_whitespace(&text);
    text
}

// ── Regex patterns (compiled once) ───────────────────────────────────────────

fn re_spaced_digits() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    // Matches a digit followed by one space and another digit, repeated.
    // E.g. "20 2 5" → "2025", "199 7" → "1997"
    R.get_or_init(|| Regex::new(r"(\d)(?: (\d))+").unwrap())
}

fn re_spaced_email() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    // Matches email-like strings where OCR has inserted spaces between every
    // character: "n a m e 9 7 @ g m a i l . c o m"
    // Each segment is `[alnum](? [alnum])*`, joined by spaced @ and dot.
    R.get_or_init(|| {
        Regex::new(r"(?i)([a-z0-9](?:(?: [a-z0-9])+)?) ?@ ?([a-z0-9](?:(?: [a-z0-9])+)?) ?\. ?([a-z](?:(?: [a-z])+)?)").unwrap()
    })
}

fn re_spaced_phone() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    // Matches phone numbers where digits and symbols are separated by spaces.
    // Handles +31 style country codes and various bracket styles.
    R.get_or_init(|| {
        Regex::new(r"(\(? ?[+]? ?\d)([ \-]\d){4,}").unwrap()
    })
}

fn re_triple_whitespace() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r" {3,}").unwrap())
}

// ── Fixers ────────────────────────────────────────────────────────────────────

fn fix_spaced_digits(text: &str) -> String {
    re_spaced_digits()
        .replace_all(text, |caps: &regex::Captures| {
            caps[0].chars().filter(|c| c.is_ascii_digit()).collect::<String>()
        })
        .into_owned()
}

fn fix_spaced_emails(text: &str) -> String {
    re_spaced_email()
        .replace_all(text, |caps: &regex::Captures| {
            let local: String = caps[1].chars().filter(|c| !c.is_whitespace()).collect();
            let domain: String = caps[2].chars().filter(|c| !c.is_whitespace()).collect();
            let tld: String = caps[3].chars().filter(|c| !c.is_whitespace()).collect();
            format!("{local}@{domain}.{tld}")
        })
        .into_owned()
}

fn fix_spaced_phone_numbers(text: &str) -> String {
    re_spaced_phone()
        .replace_all(text, |caps: &regex::Captures| {
            caps[0].chars().filter(|c| !c.is_whitespace()).collect::<String>()
        })
        .into_owned()
}

fn collapse_whitespace(text: &str) -> String {
    re_triple_whitespace()
        .replace_all(text, "  ")
        .into_owned()
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixes_spaced_year() {
        assert_eq!(fix("March 20 2 5"), "March 2025");
    }

    #[test]
    fn fixes_spaced_email() {
        let result = fix("n a m e 9 7 @ g m a i l . c o m");
        assert!(result.contains('@'), "should contain @: {result}");
        assert!(!result.contains("@ "), "should not have space after @: {result}");
    }

    #[test]
    fn fixes_spaced_phone() {
        let result = fix("( +3 1 ) 6 4 2 1 7 0 3 8 1");
        assert!(!result.contains("3 1"), "digits should be joined: {result}");
    }

    #[test]
    fn clean_text_unchanged() {
        let clean = "Hello, my name is Jane. I graduated in 2022.";
        assert_eq!(fix(clean), clean);
    }
}
