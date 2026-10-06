import {
  RESUME_CONVENTION_LOCALES,
  type ResumeConventions,
  resumeConventions,
} from '../../src/locale/index.js';
import { rustStringLiteral } from './rust-emit.js';

/**
 * `headers: &[(id, name), …],` struct-literal field for one locale's
 * `headers` record, in the object's own key order (the canonical section
 * order each locale author used). The id side is one of the fixed
 * `ResumeSectionHeaderId` literals (ASCII, no escaping needed); only the
 * localized name can contain characters `rustStringLiteral` needs to guard.
 *
 * Wrapped one tuple per line, indented to `indent`, whenever the single-line
 * form would exceed rustfmt's 100-col `max_width` — same tactic as
 * {@link rustArray}, one level deeper because this is a struct field rather
 * than a top-level const (nine tuples never fits on one line for any
 * curated locale today, but the check stays honest rather than assuming
 * that never changes).
 */
function rustHeadersField(c: ResumeConventions, indent: number): string {
  const pad = ' '.repeat(indent);
  const entries = Object.entries(c.headers).map(
    ([id, name]) => `(${JSON.stringify(id)}, ${rustStringLiteral(id, name)})`
  );
  const singleLine = `${pad}headers: &[${entries.join(', ')}],`;
  if (singleLine.length <= 100) return singleLine;
  const inner = entries.map((e) => `${pad}    ${e},`).join('\n');
  return `${pad}headers: &[\n${inner}\n${pad}],`;
}

/**
 * `pub fn resume_conventions(lang: &str) -> ResumeConventions` — the Rust
 * mirror of `locale/index.ts`'s `resumeConventions`, including its normalization
 * (first two chars, lowercased) and its English fallback for an uncurated
 * locale. Built by CALLING the real TS function once per curated locale, so the
 * two can never disagree about a header.
 *
 * `ResumeConventions.header(section_id)` looks up by the SAME canonical
 * `SectionId` Debug name (`format!("{id:?}")`, e.g. `"Summary"`) the TS
 * `ResumeSectionHeaderId` keys use — one key space on both sides, so this
 * codegen needs no separate id-name mapping table.
 */
export function rustResumeConventions(): string {
  const arms = [...RESUME_CONVENTION_LOCALES]
    .sort()
    .filter((locale) => locale !== 'en')
    .map((locale) => {
      const c = resumeConventions(locale);
      return `        ${rustStringLiteral(`resume_conventions(${locale})`, locale)} => ResumeConventions {
${rustHeadersField(c, 12)}
            date_example: ${rustStringLiteral('dateExample', c.dateExample)},
        },`;
    })
    .join('\n');
  const en = resumeConventions('en');
  return `/// Localized standard résumé section headers + a market-conventional date
/// range example.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResumeConventions {
    /// (canonical \`SectionId\` Debug name, localized header) pairs — every
    /// ordered id \`locale::resume::section_order_for\` can emit has an entry,
    /// generated from the TS \`Record<ResumeSectionHeaderId, string>\`, which
    /// the compiler refuses to build unless every locale names every id.
    headers: &'static [(&'static str, &'static str)],
    pub date_example: &'static str,
}

impl ResumeConventions {
    /// Localized header for \`section_id\`'s canonical \`SectionId\` Debug name
    /// (e.g. \`"Summary"\`, \`"Certifications"\`). Falls back to \`section_id\`
    /// itself — unreachable for any id \`locale::resume::section_order_for\`
    /// emits, since \`headers\` is total over that set, but cheaper than a
    /// panic for a caller that passes an id outside it. Takes \`section_id\`'s
    /// own lifetime (rather than \`&'static str\`) so a caller can pass a
    /// borrowed \`format!(...)\` temporary, like \`section_order_prompt_list\`
    /// does.
    pub fn header<'a>(&self, section_id: &'a str) -> &'a str {
        for &(id, name) in self.headers {
            if id == section_id {
                return name;
            }
        }
        section_id
    }

    /// Every canonical \`SectionId\` Debug name this locale names a header for,
    /// in the curated order. Lets a guard enumerate the ID axis from the data
    /// itself rather than restating it as a parallel literal list — which is
    /// how \`header\`'s silent \`section_id\` fallback could otherwise become
    /// reachable without any test noticing.
    pub fn ids(&self) -> impl Iterator<Item = &'static str> {
        self.headers.iter().map(|&(id, _)| id)
    }
}

/// Every locale \`resume_conventions\` has a curated entry for, mirrored from
/// the TS \`RESUME_CONVENTION_LOCALES\`. The companion to
/// [\`ResumeConventions::ids\`] for the OTHER axis: a guard loops over this
/// instead of hardcoding the locale list, so a locale added on the TS side
/// cannot slip past a Rust-side check that never visits it.
pub const RESUME_CONVENTION_LOCALES: &[&str] = &[${[...RESUME_CONVENTION_LOCALES]
    .sort()
    .map((l) => JSON.stringify(l))
    .join(', ')}];

/// Résumé conventions for \`lang\`, falling back to English for any locale the
/// prompt side has no curated entry for — the same normalization
/// (first two characters, lowercased) and the same fallback as the TS
/// \`resumeConventions\`.
pub fn resume_conventions(lang: &str) -> ResumeConventions {
    let key: String = lang.chars().take(2).flat_map(char::to_lowercase).collect();
    match key.as_str() {
${arms}
        // "en" and every uncurated locale.
        _ => ResumeConventions {
${rustHeadersField(en, 12)}
            date_example: ${rustStringLiteral('dateExample', en.dateExample)},
        },
    }
}`;
}
