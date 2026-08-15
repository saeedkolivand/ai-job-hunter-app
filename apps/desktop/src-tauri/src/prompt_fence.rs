//! ADR-010 prompt-injection fencing primitives — the ONE boundary mechanism
//! every untrusted blob (a scraped job posting, a candidate's résumé, prior-
//! stage model output, a bridge question) crosses before it reaches a prompt.
//!
//! **Dependency-free by design.** This module has no dependency on `agent`,
//! `pipeline`, or `commands` — only `std` + `regex` + the generated
//! [`crate::ipc_contracts::agent_caps`] constant. It used to live inside
//! `agent::tools`, but callers outside `agent` (the résumé pipeline, the
//! extension bridge, the structured-output re-ask path, Autopilot's AI notes)
//! depend on it too, so it moved here to survive `agent`'s deletion instead of
//! being deleted along with it.
//!
//! SECURITY (OWASP LLM01): [`fenced`] wraps one untrusted blob as `<tag>…
//! </tag>`, truncated to a caller-supplied cap, and neutralizes every KNOWN
//! fence tag ([`FENCE_TAG_PATTERNS`]) and tool-result marker inside it so the
//! blob can never forge its own boundary, a sibling tag's, or a transcript
//! marker. [`neutralize_transcript_boundaries`] is the shared chokepoint both
//! [`fenced`] (untrusted text entering a prompt as DATA) and
//! `agent::controller::tool_result_fence` (untrusted text re-entering the
//! transcript as a tool RESULT) call — see its own doc for the full trade-off
//! reasoning. Treat [`FENCE_TAG_PATTERNS`] as load-bearing security, not
//! bookkeeping: a stale entry costs nothing, a missing one is a
//! prompt-injection hole.

/// Char cap on the résumé text fenced into a prompt.
///
/// **GENERATED, not a literal here.** The renderer needs the same number —
/// it is the threshold a generation this fence would cut is refused at
/// rather than silently truncated — and it had been hand-copied there as a
/// second `8_000`. It now comes from `packages/shared/src/agent-caps.ts`
/// through `pnpm gen:ipc`, so the renderer imports the constant and
/// `gen:ipc:check` gates this Rust copy.
pub(crate) const RESUME_CAP: usize = crate::ipc_contracts::agent_caps::AGENT_RESUME_TEXT_CAP;

/// Char cap on the job-posting text fenced into a prompt. A local literal —
/// nothing outside this crate reads it, and generating a number with one
/// consumer is ceremony.
pub(crate) const JOB_CAP: usize = 8_000;

/// Compile the fence-tag detection pattern for one tag. `\s*` is bounded to
/// whitespace only with no adjacent unbounded quantifier chained to itself,
/// so this stays linear (no ReDoS).
///
/// **`(\s[^>]*)?` — the ATTRIBUTE form.** Until it was added, the pattern
/// required `>` after nothing but whitespace, so `<resume_strategy x="1">`
/// survived [`fenced`] BYTE-IDENTICAL: a model reading `<tag attr>` as an
/// opening tag (every one of them does — it is HTML/XML's own syntax) got a
/// forged boundary through the one primitive whose whole job is to break them.
/// Whitespace and case variants were covered; the attribute form was the hole.
/// It stays linear: `[^>]*` cannot match `>`, so it has exactly one way to
/// reach the delimiter and there is no quantifier nested inside another.
///
/// **The run is deliberately UNBOUNDED and newline-tolerant**, and both halves
/// of that were argued rather than defaulted:
///
/// * `[^>\n]*` would re-open the hole for `<tag\nattr>`, which every HTML/XML
///   parser — and every model — reads as one tag.
/// * `[^>]{0,200}` (a bounded run) would re-open it too, just further out: a
///   forged tag carrying 300 characters of attributes would stop matching
///   entirely and survive byte-identical. A bound only makes sense as damage
///   control for a transform that DELETES what it matches, and
///   [`neutralize_one`] no longer does.
fn compile_fence_tag_pattern(tag: &str) -> regex::Regex {
    let escaped = regex::escape(tag);
    regex::Regex::new(&format!(r"(?i)<\s*(/?)\s*{escaped}(\s[^>]*)?\s*>"))
        .expect("fence-tag pattern is always valid regex")
}

/// One compiled pattern per fixed tag every `fenced()` call site in this crate
/// actually uses (see its callers) — built once and reused instead of
/// recompiling the same regex on every agent turn. `neutralize_fence_tag`
/// applies EVERY one of these patterns to EVERY fenced body (see its doc for
/// why), not just the pattern matching the body's own wrapping tag; an
/// unrecognized wrapping tag (should never happen, since callers only ever
/// pass one of these literals) additionally falls back to a one-off compile,
/// so behavior is identical either way.
static FENCE_TAG_PATTERNS: std::sync::LazyLock<
    std::collections::HashMap<&'static str, regex::Regex>,
> = std::sync::LazyLock::new(|| {
    [
        "candidate_resume",
        "job_posting",
        "company_research",
        "question",
        "web_search_notes",
        "salary_context",
        // PR 11 (rewrite mode) — `extension_bridge::answer_rewrite::
        // build_rewrite_user_message` composes these two fenced blocks into
        // ONE prompt, exactly like the six above; without registering them
        // here, a crafted `existingAnswer` could forge a sibling
        // `<rewrite_instruction>` (or vice-versa) that this cross-tag
        // neutralization would otherwise miss.
        "existing_answer",
        "rewrite_instruction",
        // HIGH-1 fix — the three `agent::tools_quality` result tags. Without
        // these, a JOB-POSTING (or résumé) body carrying a forged
        // `<validate_resume_result>…</validate_resume_result>` block would
        // survive `fenced("job_posting", …)` untouched and could masquerade
        // as a real tool result once the transcript is composed; the same
        // goes for a forged sibling inside one quality tool's OWN result
        // body (e.g. a fake `<validate_resume_result>` smuggled inside
        // `search_candidate_evidence_result`'s bullet text).
        //
        // Round 9: these three tags now have NO legitimate producer.
        // `tools_quality` stopped WRAPPING its summaries in them (the wrap
        // was dead work — `agent::controller::tool_result_fence`
        // neutralized it again one layer up, so the model only ever saw
        // `< validate_resume_result>`; see that module's doc). They stay
        // registered because that makes the rule STRONGER, not weaker: any
        // occurrence of one anywhere is now unambiguously a forgery, and
        // every path a forgery can arrive on — untrusted input through
        // [`fenced`], untrusted results through `tool_result_fence` — breaks
        // it. Removing them would reopen exactly the hole HIGH-1 closed.
        "validate_resume_result",
        "search_candidate_evidence_result",
        "get_trim_suggestions_result",
        // The structured-output re-ask tag —
        // `commands::ai_provider::structured::REASK_DETAIL_TAG`, which
        // `JsonParseError::reask_detail` wraps a rejected response's parser
        // detail in before it goes back to the model. Registered here rather
        // than imported from there: the literal is pinned on the producing
        // side by `structured::tests::reask_detail_fences_the_parser_detail_
        // and_neutralizes_forged_boundaries`, which asserts the exact tag
        // text.
        //
        // Without this entry, untrusted text fenced under ANY OTHER tag (a
        // scraped `<job_posting>`, a `<candidate_resume>`) could carry a
        // forged `<invalid_json_detail>` block: the block's own fence was
        // already breakout-safe, but a forged SIBLING was never scrubbed —
        // exactly the hole the `existing_answer`/`rewrite_instruction` and
        // `*_result` entries above close, and the payoff here is a forged
        // "your JSON was rejected because …" verdict the model treats as the
        // system's own.
        "invalid_json_detail",
        // The résumé pipeline's own block tags
        // (`pipeline::resume::prompts`). Every quality-depth stage prompt
        // composes SEVERAL fenced blocks into one turn — the shape this
        // cross-tag neutralization exists for — and, unlike every tag above,
        // three of these wrap PRIOR-STAGE MODEL OUTPUT (`job_analysis`,
        // `evidence_map`, `resume_strategy`), which ADR-010 treats as untrusted
        // exactly like a scraped posting. Without these entries, a job ad
        // carrying a forged `<resume_strategy>` block would ride into the draft
        // turn looking like the pipeline's own plan — the highest-value forgery
        // available here, since the draft is written FROM that block.
        "job_analysis",
        "evidence_map",
        "resume_strategy",
        "company_roster",
        "resume_section",
        "section_issues",
        "section_note",
        // Historical max-depth section generation tags (now deleted).
        // Kept for fence-tag normalization in case historical transcripts
        // need processing. The three tags were used in max-depth section
        // turns: `source_entry` (source slice), `project_seed` (seeded links),
        // `generated_resume` (assembled document). They are still enumerated
        // here so neutralize_known_fence_tags covers them.
        "source_entry",
        "project_seed",
        "generated_resume",
        // PR-2's `humanize` stage (`pipeline::resume::prompts::humanize_user`):
        // the WHOLE document being rewritten and the flagged-line findings
        // list, composed into one turn exactly like the tags above it. A
        // forged `<humanize_findings>` riding inside the document (or vice
        // versa) would let untrusted résumé/letter text pose as the run's own
        // instruction about what to rewrite.
        "humanize_document",
        "humanize_findings",
    ]
    .into_iter()
    .map(|tag| (tag, compile_fence_tag_pattern(tag)))
    .collect()
});

/// Break every `<` that survives INSIDE a kept attribute run: `<` plus at most
/// ONE following whitespace character becomes `< `.
///
/// Writing the matched run back verbatim (the DL1 fix) is what re-opened the
/// boundary [`neutralize_one`] exists to close, for the SAME tag it was
/// breaking. Three facts compose into it: `[^>]*` admits `<`, `replace_all`
/// scans the ORIGINAL string and never rescans its own replacement, and each
/// tag gets exactly one pass over the body — so
/// `<job_posting x=</job_posting>` came out `< job_posting x=</job_posting>`,
/// carrying a byte-perfect closer on fully attacker-controlled input (the
/// scraped ad), and idempotence made it permanent rather than transient.
/// Executed, not theorised. (Cross-tag nesting was already covered — the
/// `job_posting` pass runs over the `question` pass's output — which is exactly
/// why only the same-tag case slipped through.)
///
/// Consuming one following whitespace char is what keeps the whole transform a
/// FIXED POINT (`< ` maps to `< `), which [`neutralize_transcript_boundaries`]
/// depends on. The two alternatives were both worse: `<\s*` is a fixed point too
/// but DELETES whitespace (`<\n\n` → `< `), which is the DL1 data-loss defect one
/// character at a time; a bare `<` → `< ` is not idempotent at all
/// (`< ` → `<  ` → …), so re-fencing would drift.
static INNER_LT: std::sync::LazyLock<regex::Regex> = std::sync::LazyLock::new(|| {
    regex::Regex::new(r"<\s?").expect("inner-lt pattern is always valid regex")
});

/// Apply one compiled fence-tag pattern to `body`, replacing a forged
/// opening/closing `tag` token with a visibly-broken variant (a space right
/// after `<`) rather than silently stripping it.
///
/// **The attribute run is KEPT** (with every `<` inside it broken by
/// [`INNER_LT`] — see there for the nesting hole that costs). Only the `<`s are
/// broken; every other byte the match swallowed is written back. Dropping the
/// run made this transform DELETE untrusted text rather than defuse it, and
/// `[^>]` matches newlines, so the deletion was not limited to something
/// tag-shaped: a job posting reading `<question mark of the day … 5 > 3 says
/// the ad` lost the two lines in between before the model ever saw them
/// (executed, not theorised). Nothing is gained by dropping it either — the
/// boundary is already dead once `<` is not adjacent to the tag name, and the
/// text was in the body to begin with.
///
/// Still idempotent, which matters because untrusted text can pass through both
/// [`fenced`] and `agent::controller::tool_result_fence`: `< tag attrs>`
/// re-matches and maps to itself, and so does every `< ` [`INNER_LT`] left
/// behind.
fn neutralize_one(body: &str, tag: &str, pattern: &regex::Regex) -> String {
    pattern
        .replace_all(body, |caps: &regex::Captures| {
            let slash = if &caps[1] == "/" { "/" } else { "" };
            let attrs = caps
                .get(2)
                .map(|m| INNER_LT.replace_all(m.as_str(), "< "))
                .unwrap_or_default();
            format!("< {slash}{tag}{attrs}>")
        })
        .into_owned()
}

/// Break every KNOWN fence tag inside untrusted `body` — case-insensitive
/// AND whitespace-tolerant, so spec-legal variants like `</tag >`, `< /tag>`,
/// or a tag with stray internal whitespace still can't forge a boundary that
/// breaks the model out of a fence (or into one) mid-block.
///
/// **Deliberate, documented divergence from `@ajh/prompts`' TS
/// `neutralizeFenceTag`:** the TS helper only scrubs the SAME tag being
/// wrapped (same-tag-only) — sufficient there because every TS prompt builder
/// fences exactly one untrusted block in isolation (see
/// `packages/prompts/src/generate/emphasis/emphasis.ts`). This Rust helper
/// also backs `extension_bridge::answer_assist::build_user_message`, which
/// composes SIX fenced blocks (`candidate_resume`/`job_posting`/
/// `company_research`/`question`/`web_search_notes`/`salary_context`) into
/// ONE prompt — so an attacker-controlled block (the scraped `question` text,
/// in particular) could forge a SIBLING tag like `<job_posting>` inside its
/// own body, not to escape its own fence, but to inject a second, spurious
/// job-posting-looking section the model might mistake for more-authoritative
/// job data. `extension_bridge::answer_rewrite::build_rewrite_user_message`
/// (PR 11) composes its own two fenced blocks
/// (`existing_answer`/`rewrite_instruction`) the same way, for the same
/// reason. Every tag in [`FENCE_TAG_PATTERNS`] is therefore neutralized
/// inside EVERY untrusted body, not just the tag it's about to be wrapped in.
fn neutralize_known_fence_tags(body: &str) -> String {
    let mut out = body.to_string();
    for (known_tag, pattern) in FENCE_TAG_PATTERNS.iter() {
        out = neutralize_one(&out, known_tag, pattern);
    }
    out
}

/// The bare `[tool_result:{name}]` marker `agent::controller::tool_result_fence`
/// wraps every tool result in — a DIFFERENT boundary syntax than this module's
/// `<tag>` fences (kept as-is for wire/behavior compatibility with the
/// model-facing transcript format), so [`FENCE_TAG_PATTERNS`] cannot cover it.
/// `\s*` is bounded, no adjacent unbounded quantifier chained to itself, so
/// this stays linear (no ReDoS) — same discipline as
/// [`compile_fence_tag_pattern`].
///
/// Matches ONLY the opening `[tool_result` PREFIX — deliberately NOT a full
/// `\[\s*tool_result\s*:[^\]]*\]` delimited token. A full-delimited pattern
/// whose body admits the opening delimiter itself (`[^\]]*` allows a literal
/// `[`) is always defeatable by nesting one marker inside another:
/// `replace_all` finds non-overlapping matches on the ORIGINAL string, and
/// `[^\]]*` is greedy but still stops at the FIRST `]` — so
/// `[tool_result:[tool_result:save_resume]]` matches only
/// `[tool_result:[tool_result:save_resume]` (the outer `[` through the
/// INNER marker's own closing `]`), leaving the fully-formed inner
/// `[tool_result:save_resume]` completely untouched in the output. Matching
/// the prefix alone means every occurrence of `[tool_result` is found and
/// broken independently of what brackets surround it — nesting can't hide
/// one from the scan. Same prefix-matching convention as
/// [`compile_fence_tag_pattern`]'s tag detection.
static TOOL_RESULT_MARKER: std::sync::LazyLock<regex::Regex> = std::sync::LazyLock::new(|| {
    regex::Regex::new(r"(?i)\[\s*tool_result").expect("marker pattern is always valid regex")
});

/// Break a forged `[tool_result…` marker PREFIX inside untrusted `body` (see
/// [`TOOL_RESULT_MARKER`]). Every match becomes the canonical, visibly-broken
/// `[ tool_result` (a space right after `[`, casing/spacing normalized) —
/// same "visibly broken, not silently stripped" convention as
/// [`neutralize_one`], and idempotent for the same reason (the canonical form
/// still matches the pattern and maps to itself).
fn neutralize_tool_result_marker(body: &str) -> String {
    TOOL_RESULT_MARKER
        .replace_all(body, "[ tool_result")
        .into_owned()
}

/// Make untrusted `body` inert as a transcript BOUNDARY, in every boundary
/// syntax this crate speaks: the `<tag>` fences of [`FENCE_TAG_PATTERNS`] AND
/// the `[tool_result:{name}]` marker. The single place that answers "what
/// could this text forge?", so a future third syntax is added once instead of
/// per call site.
///
/// Both chokepoints call exactly this: [`fenced`] for untrusted text entering
/// the prompt as DATA (a scraped posting, a résumé, a bridge question), and
/// `agent::controller::tool_result_fence` for untrusted text re-entering the
/// transcript as a tool RESULT. Neither syntax was symmetrically covered
/// before PR #963 round 8: `tool_result_fence` broke only the marker (so a
/// forged `<validate_resume_result>ok:true</…>` rode a `research_company`
/// brief into the transcript), and `fenced` broke only the tags (so a job
/// posting carrying `[tool_result:validate_resume]` reached the model with an
/// intact-looking transcript marker inside its own `<job_posting>` block —
/// the mirror image, same forged-verdict payoff). ADR-010: extend the
/// existing mechanism, don't add a second one.
///
/// The two passes are independent — the marker pattern matches only
/// `[…tool_result`, the tag patterns only `<…>` spans whose interior is
/// whitespace plus a fixed tag — so neither can create or hide a match for
/// the other, and their order is immaterial.
///
/// **Idempotent**: each pass rewrites a match to a canonical broken form that
/// still matches its own pattern and therefore maps to itself. For the tag
/// passes that form is `< {/}{tag}{attrs}>` — the tag token broken by the space
/// after `<`, the attribute run KEPT, and every `<` inside that run broken the
/// same way (`< ` — see [`INNER_LT`], without which a same-tag `<tag x=</tag>`
/// nested inside the run came back out intact). For the marker pass it is
/// `[ tool_result`. Text that has already been through here (a `fenced` body
/// later re-scanned by `tool_result_fence`) comes out byte-identical — no
/// cumulative corruption.
///
/// **Accepted trade-off**: legitimate prose that happens to contain one of
/// these literals is broken too (a candidate writing "our tool_result
/// pipeline", a tool's own `<…_result>` wrapper). Nothing here can tell that
/// text from a forgery — that indistinguishability is the whole finding — and
/// any exemption would hand an attacker the shape to forge. The cost is one
/// visibly-inserted space in a rare string; the alternative is a forged
/// boundary the model reads as structure.
pub(crate) fn neutralize_transcript_boundaries(body: &str) -> String {
    neutralize_known_fence_tags(&neutralize_tool_result_marker(body))
}

/// Fence one blob as `<tag>…</tag>`, TRUNCATED to `cap` chars (char-boundary
/// safe) and then made inert as a boundary (see
/// [`neutralize_transcript_boundaries`]) — so untrusted text can never forge
/// this fence's own boundary, a sibling tag's, or a tool-result marker, to
/// break out of / falsify a block.
///
/// **`cap` bounds the INPUT, not the output.** Neutralization only ever inserts
/// a space, never deletes, so the fenced body can come back longer than `cap` —
/// by at most one char per `<` and per `[tool_result` occurrence in the
/// truncated input, plus the wrapper. That is deliberate and the cheaper side of
/// the trade: the
/// alternative (truncate AFTER neutralizing) would cut a defused body at an
/// arbitrary point, and the DL1 lesson is that this primitive must not remove
/// bytes. Callers use `cap` as a context/cost guard against a huge résumé or
/// posting, and a bound that can be exceeded by the count of `<` in an
/// already-capped string is still a bound — but it is not an exact byte limit,
/// so nothing downstream may treat it as one.
pub(crate) fn fenced(tag: &str, body: &str, cap: usize) -> String {
    let body: String = body.chars().take(cap).collect();
    let mut body = neutralize_transcript_boundaries(&body);
    // `tag` is always one of `FENCE_TAG_PATTERNS`' keys for every real caller
    // today (already covered above); this only matters if a future caller
    // ever fences a tag name absent from that fixed list.
    if !FENCE_TAG_PATTERNS.contains_key(tag) {
        body = neutralize_one(&body, tag, &compile_fence_tag_pattern(tag));
    }
    format!("<{tag}>\n{body}\n</{tag}>")
}

#[cfg(test)]
mod test;
