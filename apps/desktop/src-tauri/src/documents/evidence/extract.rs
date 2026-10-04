//! `extract_evidence` — the source résumé structured into roles, projects, education and the
//! present / absent skills split, scored against one posting.
//!
//! Split out of `evidence/mod.rs` (R8's hard LOC cap); everything here moved
//! verbatim.

use crate::export::parser::parse_resume;
use crate::export::types::LineKind;
use crate::observability::Span;

use super::entry::salvage_entry_label;
use super::{
    classify_section, function_words, has_curated_function_words, split_entry,
    trailing_date_column, EvidenceRole, EvidenceSet, JobVocabulary, SectionKind,
};

/// Attach one experience line to the entry above it, opening an UNATTRIBUTED
/// bucket when no entry has parsed yet.
///
/// A résumé whose entry lines the parser does not recognise as `JobEntry` — no
/// pipe form, no two-space date column, no parenthesized span, e.g. a plain
/// "Acme Payments — Senior Backend Engineer" with the dates on the next line —
/// used to lose its ENTIRE experience section here: `checked_sub` on an empty
/// `roles` discarded every bullet silently, and the prompt was then told the
/// candidate had no experience to draw on.
///
/// The bucket carries EMPTY `company`/`title`/`dates` rather than a guessed
/// employer: inventing a company name is the one thing this module may never
/// do, and every consumer either flattens `roles[].bullets` (the agent's
/// evidence tool) or reads the fields it does have. An empty company reads as
/// "unattributed" to per-company logic instead of colliding with a real one.
fn attach_to_role(roles: &mut Vec<EvidenceRole>, vocab: &JobVocabulary, text: &str) {
    if roles.is_empty() {
        roles.push(EvidenceRole {
            company: String::new(),
            title: String::new(),
            dates: String::new(),
            bullets: Vec::new(),
        });
    }
    let role_idx = roles.len() - 1;
    let bullet_idx = roles[role_idx].bullets.len();
    let bullet = vocab.bullet(format!("r{role_idx}b{bullet_idx}"), text);
    roles[role_idx].bullets.push(bullet);
}

/// Structure the source résumé into the evidence a generation prompt is allowed
/// to draw on, scored against `job_text`.
///
/// Section membership drives everything: bullets under an experience heading
/// attach to the entry above them, bullets under a projects heading become
/// `projects`, and content lines under an education heading become `education`.
/// `skills_present`/`skills_absent` are the posting's own keywords split by
/// whether the résumé evidences them — the same split `keyword_coverage`
/// reports, in readable (unstemmed) form.
pub fn extract_evidence(source_resume: &str, job_text: &str) -> EvidenceSet {
    let span = Span::begin("evidence", "op=extract");
    let vocab = JobVocabulary::new(source_resume, job_text);
    let parsed = parse_resume(source_resume);

    let mut set = EvidenceSet::default();
    let mut section = SectionKind::Other;
    // Bullets under a heading no list recognises, held back for the last-resort
    // rescue after the loop — see there for when they are used.
    let mut unclassified_bullets: Vec<String> = Vec::new();

    for line in &parsed.lines {
        match line.kind {
            LineKind::SectionHeader => section = classify_section(&line.text),
            LineKind::JobEntry if section == SectionKind::Experience => {
                let (company, title, dates) = split_entry(line);
                set.roles.push(EvidenceRole {
                    company,
                    title,
                    dates,
                    bullets: Vec::new(),
                });
            }
            // A short line right after an entry names the role the entry's
            // label left out.
            LineKind::JobTitle if section == SectionKind::Experience => {
                if let Some(role) = set.roles.last_mut() {
                    if role.title.is_empty() {
                        role.title = line.text.clone();
                    }
                }
            }
            LineKind::Bullet => match section {
                SectionKind::Experience => attach_to_role(&mut set.roles, &vocab, &line.text),
                SectionKind::Projects => {
                    let bullet = vocab.bullet(format!("p{}", set.projects.len()), &line.text);
                    set.projects.push(bullet);
                }
                SectionKind::Education => set.education.push(line.text.clone()),
                SectionKind::Other => unclassified_bullets.push(line.text.clone()),
                // Skills and Summary bullets are classified and belong where
                // they are: a summary line is a claim about the candidate, not
                // an achievement to draw on.
                _ => {}
            },
            // A content line under Experience the parser recognised as none of
            // the above — the same "never discard the section" rule as the
            // Projects arm below, and the other half of the orphan-bullet fix
            // in [`attach_to_role`]: when the entry line itself did not parse,
            // dropping it too would lose the employer's name as well as the
            // bullets under it.
            //
            // `Contact` belongs here for exactly the reason it belongs on the
            // Education and Projects arms — and it is not an edge case, it is
            // the SHAPE this arm was written for. "Acme Payments, Berlin,
            // 2018 - 2021" is contact-shaped: `PHONE_RE` reads the date span as
            // a phone number. Without it the fix above rescued the bullets and
            // still lost the employer they belong to, which is the worse half of
            // the original bug (evidence with no attribution).
            //
            // A line that ends in a date COLUMN opens its own role, exactly as
            // a recognised `JobEntry` would. Appending it to `roles.last()` —
            // which is all [`attach_to_role`] can do — made the previous
            // employer absorb this one's header line AND every bullet under it,
            // so a second employer's work was credited to the first.
            //
            // Both gates are conservative on purpose, because the input is
            // ordinary text and the output is an employer's name:
            // [`trailing_date_column`] takes a date column, not a mentioned
            // year, and [`salvage_entry_label`] resolves an employer or returns
            // nothing. Unresolved, the role still opens (the column says an
            // entry started) but stays UNATTRIBUTED, and the label is kept as
            // its first bullet so refusing to name an employer never deletes
            // the line — the same shape the R5-F6 rescue already gives an
            // entry line it cannot attribute.
            LineKind::Text | LineKind::Contact
                if section == SectionKind::Experience && !line.text.trim().is_empty() =>
            {
                match trailing_date_column(&line.text) {
                    Some((label, dates)) => {
                        let (company, title) = salvage_entry_label(label).unwrap_or_default();
                        let unattributed = company.is_empty() && title.is_empty();
                        set.roles.push(EvidenceRole {
                            company,
                            title,
                            dates: dates.to_string(),
                            bullets: Vec::new(),
                        });
                        if unattributed {
                            attach_to_role(&mut set.roles, &vocab, label);
                        }
                    }
                    None => attach_to_role(&mut set.roles, &vocab, &line.text),
                }
            }
            // `Contact` belongs here: `export::parser` classifies any line
            // carrying a phone-shaped digit run as Contact, and a degree line
            // with a date span ("BSc Computer Science, TU Berlin, 2014 - 2018")
            // satisfies `PHONE_RE`. Without this arm the only education entries
            // that survived were the ones with no dates on them.
            LineKind::Text | LineKind::JobEntry | LineKind::Contact
                if section == SectionKind::Education && !line.text.trim().is_empty() =>
            {
                set.education.push(line.text.clone())
            }
            // `Contact` belongs here for the same reason it belongs on the
            // Education arm above: `export::parser` classifies any line carrying
            // a `github.com`/`portfolio` URL — or two `·` separators — as
            // Contact, which is precisely the owner-locked projects format
            // ("**Ledger CLI** · site · repo", then a "Rust · SQLite" stack
            // line). Without it, the only project lines that survived were the
            // bulleted ones and the prose description, so a prompt built from
            // this set was told the project had no link and no stack.
            LineKind::Text | LineKind::JobEntry | LineKind::Contact
                if section == SectionKind::Projects && !line.text.trim().is_empty() =>
            {
                let bullet = vocab.bullet(format!("p{}", set.projects.len()), &line.text);
                set.projects.push(bullet);
            }
            _ => {}
        }
    }

    // Last resort: a document with NO recognised experience section falls back
    // to the bullets under its unclassified headings.
    //
    // A heading this module cannot name is not evidence that the candidate has
    // none — `classify_section` is a fixed list of stems in seven languages, and
    // every gap in it (this round's "Beruflicher Werdegang", the next one's
    // whatever) silently emptied the set the generation prompt is allowed to
    // draw on. Gated on `roles.is_empty()` because a heading LIST is a better
    // signal than a fallback whenever there is one: a résumé with a real
    // experience section keeps its hobbies and interests out of its work
    // history. The bucket is unattributed — no employer is ever guessed — and
    // the cost when the unknown heading was really "PUBLICATIONS" is that a
    // prompt sees the candidate's own publication list as evidence, which is
    // strictly better than seeing nothing at all.
    if set.roles.is_empty() {
        for text in unclassified_bullets {
            attach_to_role(&mut set.roles, &vocab, &text);
        }
    }

    let resume_tokens = vocab.tokens(source_resume);
    // Filter BEFORE the split, on the readable form, so a function word cannot
    // land on either side: "unsere" reported as a missing SKILL is worse than
    // useless, it makes the honest gap list look broken.
    let stop = function_words(vocab.lang);
    let skill_like = |token: &&String| !stop.contains(&vocab.readable(token).as_str());
    // …but that filter only EXISTS for a curated language, and an empty slice
    // cannot say so — see [`has_curated_function_words`].
    let curated = has_curated_function_words(vocab.lang);
    // Ordered by how often the POSTING states the term, alphabetically within a
    // tie. Both lists are sized for a future truncating consumer — none reads
    // them today (`pipeline::resume::stages::strategy` calls `extract_evidence`
    // for `.roles` only); the now-deleted `agent::tools_quality::
    // compact_evidence_set` took the first N and reported only a dropped COUNT,
    // and is why a purely alphabetical order would silently hand a truncating
    // consumer the alphabetical PREFIX of the gap list — "ansible" kept,
    // "terraform" cut, and nothing downstream able to tell. Relevance-first
    // makes a truncated list the top-N by construction, so a future truncating
    // consumer needs no change.
    //
    // Determinism is unchanged, which is what the alphabetical sort was for:
    // the tiebreak is a total order, because [`display_forms`] maps each stem to
    // a token that stems back to it, so two distinct keywords cannot share one
    // display form.
    //
    // **The relevance key is switched off for an uncurated language**, which is
    // where it measures the opposite of what it claims: with no filter, the
    // terms a posting repeats most are its FILLERS ("pour" ×4, "avec" ×3), so
    // frequency sorted them to the top of the truncated GAP LIST a generation
    // prompt works from — round 8's ordering made `fr`/`es`/`it`/`nl`/`pt`
    // worse, not better. The honest degrade is to make no relevance claim at
    // all and fall back to the deterministic tiebreak.
    //
    // Emptying the lists instead was rejected on the consumer's terms: nothing
    // in `EvidenceSet` can say "unmeasured", so an empty `skills_absent` reads
    // as "no gaps" — a positive claim this module cannot support — and `lang`
    // is DETECTED, so a terse posting `whatlang` misreads would silently delete
    // an ordinary English gap list. Demoting degrades; deleting lies.
    //
    // *Residual, measured rather than assumed:* the fillers are still IN the
    // list and still consume slots, so a real requirement past the consumer's
    // cap can stay cut in BOTH orders. This stops the list ASSERTING that
    // fillers are the priorities; it does not clean the list. A bullet's `hits`
    // are unfiltered too. Only a curated `function_words` list fixes either —
    // one edit, which re-enables the relevance order with it.
    let by_relevance = |tokens: Vec<&String>| -> Vec<String> {
        let mut scored: Vec<(usize, String)> = tokens
            .into_iter()
            .map(|token| {
                let weight = if curated { vocab.weight(token) } else { 0 };
                (weight, vocab.readable(token))
            })
            .collect();
        scored.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(&b.1)));
        scored.into_iter().map(|(_, display)| display).collect()
    };
    set.skills_present = by_relevance(
        vocab
            .keywords
            .intersection(&resume_tokens)
            .filter(skill_like)
            .collect(),
    );
    set.skills_absent = by_relevance(
        vocab
            .keywords
            .difference(&resume_tokens)
            .filter(skill_like)
            .collect(),
    );

    // Codes and counts only — never résumé or posting text (ADR-027).
    span.end_with(
        &format!(
            "roles={} projects={} skills_present={} skills_absent={}",
            set.roles.len(),
            set.projects.len(),
            set.skills_present.len(),
            set.skills_absent.len()
        ),
        true,
    );
    set
}
