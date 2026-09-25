use super::*;
/// Every `"error":` STRING LITERAL mcp.rs's own source writes directly — never `agent_call`'s
/// `pub(super)` sentinels (`ERR_UNKNOWN_COMMAND`/`ERR_NOT_EXPOSED`/`ERR_CONFIRMATION_REQUIRED`),
/// referenced by path there and never respelled here. A test-only fixture (item 24): nothing in
/// production reads it, only the two tests below.
const MCP_SENTINELS: &[&str] = &[
    "wrong_tool",
    "tier_not_enabled",
    "result_too_large",
    "server_busy",
    "shutting_down",
];

#[test]
fn instructions_name_every_mcp_only_sentinel() {
    // item 24 — wrong_tool/result_too_large are MCP-only outcomes named nowhere else.
    for sentinel in MCP_SENTINELS {
        assert!(
            INSTRUCTIONS.contains(sentinel),
            "INSTRUCTIONS must name MCP-only sentinel `{sentinel}`"
        );
    }
}

/// Find the next `"error"` key in `source` at or after `from` whose value is a string literal,
/// tolerating ANY amount of whitespace (including a newline, i.e. rustfmt splitting key and value
/// across lines) between `"error"`, `:`, and the opening quote — CodeRabbit, PR #1092: the prior
/// scanner matched only the exact spelling `"error": "` (one space), so `"error":"x"` or a
/// line-split write would silently produce NO match while the "found is non-empty" sanity check
/// stayed green on whatever it DID happen to catch elsewhere in the file. Returns the literal's
/// value and the index just past its closing quote, so the caller can resume scanning from there;
/// a `"error"` occurrence whose value isn't a string (e.g. `"error": some_const`) is skipped, not
/// treated as a scan failure.
fn next_error_literal(source: &str, from: usize) -> Option<(&str, usize)> {
    let bytes = source.as_bytes();
    let mut search_from = from;
    loop {
        let key_pos = source[search_from..].find("\"error\"")?;
        let after_key = search_from + key_pos + "\"error\"".len();
        let mut i = after_key;
        while bytes.get(i).is_some_and(u8::is_ascii_whitespace) {
            i += 1;
        }
        if bytes.get(i) != Some(&b':') {
            search_from = after_key;
            continue;
        }
        i += 1;
        while bytes.get(i).is_some_and(u8::is_ascii_whitespace) {
            i += 1;
        }
        if bytes.get(i) != Some(&b'"') {
            search_from = after_key;
            continue;
        }
        let value_start = i + 1;
        let value_end = value_start + source[value_start..].find('"')?;
        return Some((&source[value_start..value_end], value_end + 1));
    }
}

#[test]
fn every_error_literal_in_mcp_source_is_named_in_mcp_sentinels() {
    // item 24 — a drift guard: every `"error": "..."` string literal this file's own source
    // writes must be a member of MCP_SENTINELS (never `agent_call`'s shared sentinels, which are
    // referenced by path, not respelled here).
    let mut idx = 0;
    let mut found = Vec::new();
    while let Some((value, next)) = next_error_literal(SOURCE, idx) {
        found.push(value);
        idx = next;
    }
    assert!(
        !found.is_empty(),
        "sanity: the scanner must find at least one literal"
    );
    for f in &found {
        assert!(
            MCP_SENTINELS.contains(f),
            "mcp.rs writes \"error\": \"{f}\" but MCP_SENTINELS doesn't name it"
        );
    }
}

#[test]
fn build_instructions_appends_one_sentence_per_enabled_tier_and_never_duplicates_the_base() {
    let none = build_instructions(Tier::Read);
    let reversible = build_instructions(Tier::Reversible);
    let irreversible = build_instructions(Tier::Irreversible);
    // Was an exact equality against bare INSTRUCTIONS; issue #1143 appends the derived sentinel
    // table at EVERY tier, so the invariant this test owns is "leads with the base text and adds
    // no TIER notice", not "is byte-identical to the base text".
    assert!(
        none.starts_with(INSTRUCTIONS),
        "must still lead with the base text: {none}"
    );
    assert!(
        !none.contains("tier is enabled"),
        "no flags must append no tier notice: {none}"
    );
    assert!(reversible.starts_with(INSTRUCTIONS));
    assert!(reversible.contains("reversible write tier is enabled"));
    assert!(
        !reversible.contains("irreversible tier is enabled"),
        "the irreversible notice must not appear at the reversible tier: {reversible}"
    );
    assert!(irreversible.contains("reversible write tier is enabled"));
    assert!(irreversible.contains("irreversible tier is enabled"));
    assert_eq!(
        irreversible.matches("loopback bridge").count(),
        1,
        "must append, never duplicate, the base INSTRUCTIONS text"
    );
}

/// Issue #1143 — the shipped instructions named 2 of the ~10 sentinels this CLI can return, so a
/// client hitting `pairing_token_unavailable`/`pairing_rejected` (a moved data dir, a re-pair)
/// got an unexplained string. Two-sided on purpose: every row must be REACHABLE from the final
/// text, and the one deliberate omission is asserted ABSENT and anchored to its own const, so
/// widening the exclusion silently is what fails here rather than passing quietly.
#[test]
fn every_error_sentinel_is_named_in_the_final_instructions_except_the_pre_protocol_one() {
    let text = build_instructions(Tier::Read);
    for (sentinel, meaning) in ERROR_SENTINELS {
        if *sentinel == ERR_RUNTIME_UNAVAILABLE {
            assert!(
                !text.contains(sentinel),
                "`{sentinel}` fires before the protocol starts (stderr + exit 2), so no tool \
                 result can carry it and the instructions must not promise it: {text}"
            );
            continue;
        }
        assert!(
            text.contains(sentinel),
            "instructions must name every sentinel a tool result can carry, missing `{sentinel}`"
        );
        // The MEANING travels with the name for the rows the skip list doesn't claim — a bare
        // name list would leave the client exactly as unable to recover as before. The escape
        // hatch is the SAME literal list the builder filters on (MEDIUM fix, review round 4): it
        // used to be `INSTRUCTIONS.contains(sentinel)`, so a sentinel the prose merely MENTIONED
        // satisfied both the filter and its own test.
        assert!(
            text.contains(meaning) || EXPLAINED_IN_PROSE.contains(sentinel),
            "`{sentinel}` is only listed, never explained: {text}"
        );
    }
}

/// The filter, not just the table: a sentinel the base prose already explains must NOT be
/// re-listed. Mutating `sentinel_table`'s `!EXPLAINED_IN_PROSE.contains(name)` away is what this
/// catches — otherwise a raw dump would repeat `app_not_running` in the same string twice.
#[test]
fn the_sentinel_table_skips_rows_the_base_prose_already_explains() {
    let text = build_instructions(Tier::Read);
    for already_named in EXPLAINED_IN_PROSE {
        assert_eq!(
            text.matches(already_named).count(),
            INSTRUCTIONS.matches(already_named).count(),
            "`{already_named}` must not be repeated by the derived table: {text}"
        );
    }
}

/// The skip list against a SECOND hand-written literal list — the repo's standing pairing rule
/// (a test that loops over the table it is checking can only catch additions). Removing a name
/// here is what re-adds a redundant table row for a sentinel the prose already explains; the
/// loop-over-EXPLAINED_IN_PROSE tests around this one cannot see that by construction.
#[test]
fn the_skip_list_matches_a_hand_written_literal_list() {
    assert_eq!(
        EXPLAINED_IN_PROSE,
        ["app_not_running", "app_not_located"],
        "changing the skip list means re-reading the prose: a name belongs here only if \
         INSTRUCTIONS says what the sentinel IS and what to do about it, not merely mentions it"
    );
}

/// The skip list is hand-written, so it needs both directions pinned (MEDIUM fix, review round 4
/// — a hand-written list nothing checks is exactly the drift the old substring filter had).
/// Forward: every name on it is a REAL `ERROR_SENTINELS` row (otherwise it is inert) that the
/// base prose really does mention. Backward: every sentinel a tool result can carry is either on
/// the list or has its own derived row — no third state.
#[test]
fn every_skip_list_name_is_a_real_sentinel_the_prose_names_and_every_other_row_is_in_the_table() {
    for name in EXPLAINED_IN_PROSE {
        assert!(
            ERROR_SENTINELS.iter().any(|(n, _)| n == name),
            "`{name}` is not an ERROR_SENTINELS row, so skipping it does nothing"
        );
        assert!(
            INSTRUCTIONS.contains(name),
            "the base prose must actually explain `{name}` — it is not even mentioned"
        );
    }
    let table = build_instructions(Tier::Read)
        .strip_prefix(INSTRUCTIONS)
        .expect("the derived table is appended to the base prose")
        .to_string();
    for (sentinel, meaning) in ERROR_SENTINELS {
        if *sentinel == ERR_RUNTIME_UNAVAILABLE {
            continue;
        }
        if EXPLAINED_IN_PROSE.contains(sentinel) {
            assert!(
                !table.contains(sentinel),
                "`{sentinel}` is claimed as explained by the prose, so the table must skip it"
            );
        } else {
            assert!(
                table.contains(sentinel) && table.contains(meaning),
                "`{sentinel}` is neither claimed by the skip list nor listed with its meaning: \
                 {table}"
            );
        }
    }
}

/// The row `connection_lost` lost to the old substring filter — the prose names it only inside
/// "don't retry in a loop", which never says what it IS. Anchored to the sentinel that produced
/// the finding rather than to the list, so removing it from the table fails here even if someone
/// adds it to `EXPLAINED_IN_PROSE` at the same time.
#[test]
fn connection_lost_gets_its_own_table_row_because_the_prose_only_mentions_it() {
    let text = build_instructions(Tier::Read);
    assert!(
        text.matches(ERR_CONNECTION_LOST).count()
            > INSTRUCTIONS.matches(ERR_CONNECTION_LOST).count(),
        "a merely-mentioned sentinel must still be defined by the table: {text}"
    );
    let (_, meaning) = ERROR_SENTINELS
        .iter()
        .find(|(n, _)| *n == ERR_CONNECTION_LOST)
        .expect("connection_lost is a sentinel");
    assert!(text.contains(meaning), "with its meaning: {text}");
}

#[test]
fn instructions_notices_are_worded_by_tier_not_by_the_literal_flag_typed() {
    // item 27 — launched with ONLY --allow-irreversible; Tier::Irreversible implies the
    // reversible tier too, so BOTH notices append, but neither may claim a flag never typed.
    let text = build_instructions(Tier::from_flags(false, true));
    assert!(
        !text.contains("--allow-reversible") && !text.contains("--allow-irreversible"),
        "notices must be worded by TIER, not by the literal flag: {text}"
    );
    assert!(text.contains("reversible write tier is enabled"));
    assert!(text.contains("irreversible tier is enabled"));
}
