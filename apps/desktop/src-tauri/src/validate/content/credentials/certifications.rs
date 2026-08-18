//! A2b — certifications. A curated issuer list, a curated acronym list and a
//! curated set of the roles a certification certifies somebody to fill; never
//! an inference from capitalisation.

use std::collections::HashSet;
use std::sync::LazyLock;

use regex::Regex;

use super::super::{contains_phrase, Section};
use super::contains_upper_acronym;

// ── The trigger set ────────────────────────────────────────────────────────

/// How far apart an issuer and a certification word may sit, on the SOURCE
/// side, and still name one certification.
///
/// Source-side only — see [`Side`]. The claims side requires ADJACENCY, and the
/// asymmetry is the point: a window this wide reads "certified the release on
/// AWS each Thursday" as a credential, which is fine when it SPARES a claim and
/// a false accusation when it makes one.
pub const CERT_ISSUER_WINDOW_CHARS: usize = 60;

/// A line at or under this length is quoted WHOLE as the evidence for a
/// certification finding: an entry in a Certifications section is already the
/// exact span the user has to look at, and "AWS Certified" alone loses the half
/// of the name that says which certification.
pub const CERT_EVIDENCE_LINE_CHARS: usize = 120;

/// Which document a certification scan is reading. The two sides ask different
/// questions of the same text and the difference is deliberate — see
/// [`cert_claims_on_line`].
#[derive(Clone, Copy, PartialEq, Eq)]
enum Side {
    /// The generated document: a certification here is a CLAIM, so the shape
    /// must be unambiguous.
    Claims,
    /// The candidate's own résumé: whatever it names may only ever spare a
    /// claim, so the reading is as generous as it can be made.
    Source,
}

/// Punctuation that may sit between an issuer and a certification word without
/// breaking their adjacency: `AWS Certified …`, `Microsoft Certified: Azure
/// …`, `Zertifizierter Kubernetes-Administrator`.
///
/// Anything else between them is a WORD, and a word is what tells "AWS
/// Certified Solutions Architect" (a credential) from "certified the release on
/// AWS" (a Thursday).
const CERT_ADJACENCY_PUNCTUATION: &[char] = &[
    '-', '\u{2013}', '\u{2014}', ':', ',', '(', ')', '.', '\u{b7}', '|', '/', '\u{2019}', '\'',
];

/// Certification acronyms, each mapped to the KEY its expansion resolves to and
/// to the expansion itself.
///
/// **One namespace, three columns, and the middle one is load-bearing.** An
/// earlier version emitted `cka` from the acronym pass and `kubernetes` from
/// the issuer pass, and `unsupported_certs` compares keys: a source writing
/// `Certified Kubernetes Administrator` therefore did not support a generated
/// `CKA`, nor the reverse. Both directions were measured as false Criticals.
/// The key is the issuer's, so a certification from an issuer the source
/// already holds a certification from is spared — deliberately generous, and
/// the alternative is exactly the two-namespace bug this column exists to kill.
///
/// The expansion is matched on the SOURCE side only, for the acronyms whose
/// long form carries no issuer token of its own (`CISSP`, `PMP`, `CFA`): without
/// it a source spelling the certification out in full would not support the
/// output's abbreviation of it.
///
/// Curated, and short on purpose. Every entry is a token that means one thing
/// on a résumé in any language — which is why it is checkable, and why the
/// list may not be grown by pattern-matching on capitalisation. Two deliberate
/// exclusions worth naming: `CSM` (also "customer success manager") and `CPA`
/// (also "cost per acquisition" on a marketing résumé). Both are real
/// certifications; neither is UNAMBIGUOUS, and an ambiguous entry here is a
/// false Critical waiting for the first marketing CV.
const CERT_ACRONYMS: &[(&str, &str, &str)] = &[
    ("PMP", "pmi", "project management professional"),
    ("CAPM", "pmi", "certified associate in project management"),
    ("PRINCE2", "prince2", ""),
    ("TOGAF", "togaf", ""),
    ("ITIL", "itil", ""),
    (
        "CISSP",
        "isc2",
        "certified information systems security professional",
    ),
    ("CCSP", "isc2", "certified cloud security professional"),
    ("CISM", "isaca", "certified information security manager"),
    ("CISA", "isaca", "certified information systems auditor"),
    ("CEH", "eccouncil", "certified ethical hacker"),
    (
        "OSCP",
        "offsec",
        "offensive security certified professional",
    ),
    ("CCNA", "cisco", "cisco certified network associate"),
    ("CCNP", "cisco", "cisco certified network professional"),
    ("CCIE", "cisco", "cisco certified internetwork expert"),
    ("CKA", "kubernetes", "certified kubernetes administrator"),
    (
        "CKAD",
        "kubernetes",
        "certified kubernetes application developer",
    ),
    (
        "CKS",
        "kubernetes",
        "certified kubernetes security specialist",
    ),
    ("RHCE", "redhat", "red hat certified engineer"),
    ("RHCSA", "redhat", "red hat certified system administrator"),
    ("CFA", "cfa", "chartered financial analyst"),
    ("FRM", "frm", "financial risk manager"),
    ("PSM", "scrum", "professional scrum master"),
    ("PSPO", "scrum", "professional scrum product owner"),
];

/// Certification issuers, each mapped to the KEY a claim is compared on — the
/// same namespace [`CERT_ACRONYMS`]' middle column uses.
///
/// Two spellings of one issuer share a key, so a source writing "Amazon Web
/// Services" supports a generated "AWS Certified …".
const CERT_ISSUERS: &[(&str, &str)] = &[
    ("amazon web services", "aws"),
    ("amazon", "aws"),
    ("aws", "aws"),
    ("microsoft", "microsoft"),
    ("azure", "microsoft"),
    ("google cloud", "google"),
    ("google", "google"),
    ("cisco", "cisco"),
    ("oracle", "oracle"),
    ("comptia", "comptia"),
    ("red hat", "redhat"),
    ("redhat", "redhat"),
    ("kubernetes", "kubernetes"),
    ("cncf", "kubernetes"),
    ("linux foundation", "linux-foundation"),
    ("salesforce", "salesforce"),
    ("scrum alliance", "scrum"),
    ("scrum", "scrum"),
    ("pmi", "pmi"),
    ("isaca", "isaca"),
    ("isc2", "isc2"),
    ("ec-council", "eccouncil"),
    ("offensive security", "offsec"),
    ("hashicorp", "hashicorp"),
    ("terraform", "hashicorp"),
    ("docker", "docker"),
    ("mongodb", "mongodb"),
    ("databricks", "databricks"),
    ("snowflake", "snowflake"),
    ("tableau", "tableau"),
    ("vmware", "vmware"),
    ("juniper", "juniper"),
    ("sap", "sap"),
    ("six sigma", "six-sigma"),
];

/// The word "certified"/"certification" in the languages this pipeline writes.
///
/// German compounds it (`Zertifizierter`, `Zertifizierung`), so the German
/// entries are matched as PREFIXES via the regex's own `\w*` tail rather than
/// spelled out one inflection at a time.
static CERT_WORD_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)\b(certified|certification|certificate|zertifizier\w*|zertifikat\w*|certifi[ée]e?s?|certificado\w*|certificaci[óo]n|certificat[oi]|certificazion\w*|gecertificeerd|certificaat|certifica[çc][ãa]o)\b",
    )
    .unwrap()
});

/// Every issuer token, longest spelling first — `\b` does not decide an
/// alternation, and "amazon" would otherwise win against "amazon web services".
static CERT_ISSUER_RE: LazyLock<Regex> = LazyLock::new(|| {
    let mut names: Vec<&str> = CERT_ISSUERS.iter().map(|(name, _)| *name).collect();
    names.sort_by_key(|n| std::cmp::Reverse(n.len()));
    Regex::new(&format!(r"(?i)\b({})\b", names.join("|"))).unwrap()
});

/// How much text after a credential is read looking for the role it names.
pub const CERT_ROLE_NOUN_WINDOW_CHARS: usize = 48;

/// The role a certification certifies somebody to BE.
///
/// Adjacency alone kills the verb ("Certified the release on AWS"), but not the
/// ADJECTIVE, which is grammatically identical to a real credential: `Docker
/// Certified images`, `Red Hat certified build images`, `VMware certified
/// storage arrays`, `Certified Scrum team`. Those are vendor marketing terms
/// applied to PRODUCTS, and seven of seven fired.
///
/// A real certification names a person: Solutions ARCHITECT, Kubernetes
/// ADMINISTRATOR, Scrum MASTER, Network ASSOCIATE. So the claims side requires
/// one of these within [`CERT_ROLE_NOUN_WINDOW_CHARS`] after the credential —
/// "team", "images", "arrays" and "clusters" are not people.
///
/// Matched as SUBSTRINGS, because German compounds the role straight onto the
/// subject: `Netzwerkadministrator`, `Netzwerkexperte`. Every entry is long
/// enough that a substring match is not a coincidence.
const CERT_ROLE_NOUNS: &[&str] = &[
    "architect",
    "administrator",
    "administrateur",
    "engineer",
    "ingénieur",
    "ingenieur",
    "associate",
    "professional",
    "professionnel",
    "practitioner",
    "developer",
    "entwickler",
    "expert",
    "experte",
    "specialist",
    "spezialist",
    "master",
    "owner",
    "analyst",
    "consultant",
    "auditor",
    "manager",
];

/// True when a role noun follows the credential closely enough to be its
/// subject.
fn names_a_certified_role(line: &str, span_end: usize) -> bool {
    let tail: String = line[span_end..]
        .chars()
        .take(CERT_ROLE_NOUN_WINDOW_CHARS)
        .collect::<String>()
        .to_lowercase();
    CERT_ROLE_NOUNS.iter().any(|role| tail.contains(role))
}

/// One certification a document names.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CertClaim {
    /// Every key this claim may be recognised by — an acronym contributes both
    /// its own spelling and its issuer's key, so the two passes cannot end up
    /// in different namespaces again. A claim is unsupported only when NONE of
    /// them is in the source.
    pub keys: Vec<String>,
    /// The span the document wrote — the evidence a finding quotes.
    pub raw: String,
    pub section: Option<String>,
}

/// True when nothing but whitespace and [`CERT_ADJACENCY_PUNCTUATION`] separates
/// the two spans `[a_end, b_start)` — i.e. the issuer and the certification
/// word are neighbouring TOKENS, in either order.
fn spans_are_adjacent(line: &str, first_end: usize, second_start: usize) -> bool {
    if first_end > second_start {
        return true; // overlapping: "Zertifizierter" IS the certification word
    }
    line[first_end..second_start]
        .chars()
        .all(|c| c.is_whitespace() || CERT_ADJACENCY_PUNCTUATION.contains(&c))
}

/// The certifications one line names, as `(keys, evidence span)`.
///
/// Two passes, because certifications are written two ways and only one of them
/// carries a word: a bare acronym (`PMP`), and an issuer beside a certification
/// word in either order (`AWS Certified …`, `Certified Kubernetes
/// Administrator`, `Zertifizierter Scrum Master`).
///
/// ## The two sides read differently, and that is the fix
///
/// "Certified" is a PAST-TENSE VERB at least as often as it is an adjective on
/// a résumé. Accepting any issuer token within [`CERT_ISSUER_WINDOW_CHARS`] of
/// it turned "Certified the release on AWS each Thursday" and "Migrated 40
/// services to Docker and certified each image against CIS" into invented
/// credentials — four of four realistic bullets fired, and the user was told to
/// delete a real achievement.
///
/// So [`Side::Claims`] requires the two to be ADJACENT tokens, which is what
/// "AWS Certified" is and what "certified … on AWS" is not, while
/// [`Side::Source`] keeps the wide window, and drops the role-noun test with
/// it.
///
/// `Source ⊇ Claims` therefore holds for gaps up to
/// [`CERT_ISSUER_WINDOW_CHARS`] — not unconditionally, because adjacency admits
/// an unbounded run of punctuation that the window would cut off. A credential
/// written with more than 60 characters of dashes between its issuer and its
/// certification word is not a document anyone has. Within that bound the
/// asymmetry runs the way it must: a source whose prose merely mentions
/// certifying something on AWS SPARES a generated AWS certification, which is a
/// missed check rather than a false accusation.
fn cert_claims_on_line(line: &str, side: Side) -> Vec<(Vec<String>, String)> {
    let mut out = Vec::new();
    for (acronym, key, expansion) in CERT_ACRONYMS {
        let found = match side {
            Side::Claims => contains_upper_acronym(line, acronym),
            // Any casing, plus the long form: a source writing "cissp" in a
            // skills line, or spelling the certification out in full, still
            // supports the claim.
            Side::Source => {
                let lower = line.to_lowercase();
                contains_phrase(&lower, &acronym.to_lowercase())
                    || (!expansion.is_empty() && contains_phrase(&lower, expansion))
            }
        };
        if found {
            // Issuer key FIRST: `unsupported_certs` deduplicates on
            // `keys[0]`, and a certification named in BOTH forms — the acronym
            // on one line, the issuer phrase on another — produced two findings
            // about one credential while the two passes disagreed about which
            // element that was.
            out.push((
                vec![(*key).to_string(), acronym.to_lowercase()],
                (*acronym).to_string(),
            ));
        }
    }
    for word in CERT_WORD_RE.find_iter(line) {
        for issuer in CERT_ISSUER_RE.captures_iter(line) {
            let Some(name) = issuer.get(1) else { continue };
            let (first_end, second_start) = if word.end() <= name.start() {
                (word.end(), name.start())
            } else {
                (name.end(), word.start())
            };
            let end = word.end().max(name.end());
            let accepted = match side {
                Side::Claims => {
                    spans_are_adjacent(line, first_end, second_start)
                        && names_a_certified_role(line, end)
                }
                Side::Source => second_start.saturating_sub(first_end) <= CERT_ISSUER_WINDOW_CHARS,
            };
            if !accepted {
                continue;
            }
            let key = CERT_ISSUERS
                .iter()
                .find(|(spelling, _)| spelling.eq_ignore_ascii_case(name.as_str()))
                .map(|(_, key)| (*key).to_string())
                .unwrap_or_else(|| name.as_str().to_lowercase());
            let trimmed = line.trim();
            let raw = if trimmed.chars().count() <= CERT_EVIDENCE_LINE_CHARS {
                trimmed.to_string()
            } else {
                line[word.start().min(name.start())..end].trim().to_string()
            };
            out.push((vec![key], raw));
        }
    }
    out
}

/// Every certification the generated document names.
pub fn cert_claims(sections: &[Section]) -> Vec<CertClaim> {
    let mut out = Vec::new();
    for section in sections {
        for line in &section.lines {
            for (keys, raw) in cert_claims_on_line(&line.text, Side::Claims) {
                out.push(CertClaim {
                    keys,
                    raw,
                    section: section.heading.clone(),
                });
            }
        }
    }
    out
}

/// Every certification key the SOURCE names — the lenient side.
pub fn source_cert_keys(source: &str) -> HashSet<String> {
    source
        .lines()
        .flat_map(|line| cert_claims_on_line(line, Side::Source))
        .flat_map(|(keys, _)| keys)
        .collect()
}

/// Certifications the generated document names that the source never does.
pub fn unsupported_certs(generated_sections: &[Section], source: &str) -> Vec<CertClaim> {
    let known = source_cert_keys(source);
    let mut seen = HashSet::new();
    cert_claims(generated_sections)
        .into_iter()
        .filter(|claim| !claim.keys.iter().any(|key| known.contains(key)))
        .filter(|claim| {
            claim
                .keys
                .first()
                .is_some_and(|key| seen.insert(key.clone()))
        })
        .collect()
}
