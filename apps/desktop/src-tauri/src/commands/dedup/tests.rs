use super::{clamp_split_request, MAX_DEDUP_KEY_BYTES, MAX_OTHER_KEYS};

#[test]
fn clamp_caps_other_keys_at_the_limit() {
    // A caller that bypassed the Zod `.max(32)` sends 100 keys → clamped to
    // 32, so the insert is bounded (CWE-770).
    let others: Vec<String> = (0..100).map(|i| format!("k{i}")).collect();
    let (member, clamped) = clamp_split_request("member", &others).expect("valid request");
    assert_eq!(member, "member");
    assert_eq!(
        clamped.len(),
        MAX_OTHER_KEYS,
        "other_keys must be capped at the server limit"
    );
}

#[test]
fn clamp_trims_drops_blanks_and_self_pairs() {
    let mixed = vec![
        "  ".to_string(),     // blank → dropped
        "member".to_string(), // self-pair → dropped
        "  real  ".to_string(),
    ];
    let (_, clamped) = clamp_split_request("member", &mixed).expect("valid request");
    assert_eq!(
        clamped,
        vec!["real".to_string()],
        "trimmed, blank + self dropped"
    );
}

#[test]
fn clamp_collapses_duplicate_other_keys_preserving_first_seen_order() {
    // The SAME key repeated (interleaved) collapses to ONE entry, and the
    // surviving order is first-seen: `b` before `a` because `b` appears first.
    let keys = vec![
        "b".to_string(),
        "a".to_string(),
        "b".to_string(), // dup of the first-seen `b`
        "a".to_string(), // dup of `a`
        "c".to_string(),
    ];
    let (_, clamped) = clamp_split_request("member", &keys).expect("valid request");
    assert_eq!(
        clamped,
        vec!["b".to_string(), "a".to_string(), "c".to_string()],
        "duplicates collapse to one entry, first-seen order preserved"
    );
}

#[test]
fn clamp_dedup_before_cap_yields_full_distinct_capacity() {
    // 40 DISTINCT keys, with the first key hammered 20 extra times
    // interleaved. De-dup runs BEFORE the 32-cap, so the repeats collapse and
    // do NOT steal slots — the result is the FULL 32 distinct keys (d0..d31),
    // not fewer.
    let mut keys: Vec<String> = Vec::new();
    for i in 0..40 {
        keys.push(format!("d{i}"));
        if i < 20 {
            keys.push("d0".to_string()); // repeatedly hammer one key
        }
    }
    let (_, clamped) = clamp_split_request("member", &keys).expect("valid request");
    assert_eq!(
        clamped.len(),
        MAX_OTHER_KEYS,
        "dedup-before-cap must yield the FULL 32 distinct slots, not fewer"
    );
    let unique: std::collections::HashSet<&String> = clamped.iter().collect();
    assert_eq!(
        unique.len(),
        clamped.len(),
        "all surviving keys are distinct"
    );
    // The cap keeps the first 32 DISTINCT keys in first-seen order.
    assert_eq!(clamped.first().map(String::as_str), Some("d0"));
    assert!(
        clamped.contains(&"d31".to_string()),
        "a distinct key up to the cap survives despite the repeats"
    );
    assert!(
        !clamped.contains(&"d32".to_string()),
        "distinct keys beyond the cap are dropped"
    );
}

#[test]
fn clamp_deduplicates_repeated_keys_before_the_cap() {
    // 33 entries where two are duplicates (k0 and k1 each appear twice) → 31
    // DISTINCT keys. De-dup runs BEFORE the 32-cap, so all 31 are kept
    // (first-seen order) and a repeated key never wastes a slot.
    let mut keys: Vec<String> = (0..31).map(|i| format!("k{i}")).collect();
    keys.push("k0".to_string()); // duplicate
    keys.push("k1".to_string()); // duplicate
    assert_eq!(keys.len(), 33);

    let (_, clamped) = clamp_split_request("member", &keys).expect("valid request");
    assert_eq!(
        clamped.len(),
        31,
        "the two duplicate keys must not consume slots — 31 distinct pairs"
    );
    // No duplicates survive, and first-seen order is preserved.
    let unique: std::collections::HashSet<&String> = clamped.iter().collect();
    assert_eq!(unique.len(), clamped.len(), "no duplicate key survives");
    assert_eq!(clamped.first().map(String::as_str), Some("k0"));
}

#[test]
fn clamp_byte_caps_oversized_keys() {
    let big_member = "m".repeat(500);
    let big_other = "o".repeat(500);
    let (member, clamped) = clamp_split_request(&big_member, &[big_other]).expect("valid request");
    assert!(
        member.len() <= MAX_DEDUP_KEY_BYTES,
        "member is byte-clamped"
    );
    assert_eq!(clamped.len(), 1);
    assert!(
        clamped[0].len() <= MAX_DEDUP_KEY_BYTES,
        "other key is byte-clamped"
    );
}

#[test]
fn clamp_byte_cap_cuts_on_a_utf8_char_boundary() {
    // A multi-byte (UTF-8) key well over the cap must be truncated on a char
    // boundary — never mid-codepoint (`String::truncate` would PANIC if the
    // char-boundary walk-back in `clamp_bytes` regressed). Mirrors the
    // salary-field clamp regression net (job_preferences/test.rs).
    let euros = "€".repeat(150); // 150 × 3 bytes = 450 bytes, over the 200 cap
                                 // member path
    let (member, _) = clamp_split_request(&euros, &["distinct".to_string()])
        .expect("a distinct other key keeps this a valid request");
    assert!(member.len() <= MAX_DEDUP_KEY_BYTES, "member byte-clamped");
    assert!(
        member.is_char_boundary(member.len()),
        "member clamp must cut on a char boundary (valid UTF-8)"
    );
    // other_keys path
    let (_, clamped) = clamp_split_request("member", &[euros]).expect("valid request");
    assert_eq!(clamped.len(), 1);
    assert!(
        clamped[0].len() <= MAX_DEDUP_KEY_BYTES,
        "other key byte-clamped"
    );
    assert!(
        clamped[0].is_char_boundary(clamped[0].len()),
        "other-key clamp must cut on a char boundary (valid UTF-8)"
    );
}

#[test]
fn clamp_rejects_empty_member_or_no_usable_others() {
    // Empty/whitespace member → nothing to record.
    assert!(clamp_split_request("   ", &["k".to_string()]).is_none());
    // Others all blank / only a self-pair → nothing to record.
    assert!(clamp_split_request("member", &["  ".to_string()]).is_none());
    assert!(clamp_split_request("member", &["member".to_string()]).is_none());
}
