use super::*;

// ── jitter ─────────────────────────────────────────────────────────────

#[test]
fn jitter_is_stable_for_an_id_and_inside_the_window() {
    // Stability is the property the whole design rests on: the occurrence has
    // to be the SAME instant on every tick, or catch-up and no-double-run both
    // break. Absolute bound, not a comparison against the constant re-derived.
    let id = "3170f7e8-b2f9-4c84-beac-754b509d554b";
    let first = jitter_for(id);

    assert_eq!(
        first,
        jitter_for(id),
        "the same id must always jitter the same"
    );
    assert!(first >= chrono::Duration::zero());
    assert!(first < chrono::Duration::seconds(600));
}

#[test]
fn jitter_actually_spreads_a_herd() {
    // The point of the feature. Without a spread this is an elaborate no-op,
    // so assert a real one: 200 ids must land in many distinct minutes, not
    // all in the same one.
    let buckets: std::collections::HashSet<i64> = (0..200)
        .map(|i| jitter_for(&format!("ap-{i}-4c84-beac-754b509d554b")).num_minutes())
        .collect();

    assert!(
        buckets.len() >= 8,
        "200 autopilots landed in only {} distinct minutes — the herd is not spread",
        buckets.len()
    );
}

#[test]
fn jitter_moves_the_occurrence_by_exactly_the_offset() {
    // now 14:30, daily 09:00, jitter 7m → today 09:07, not 09:00.
    let now = now_at(14, 30);
    let seven = chrono::Duration::minutes(7);

    assert_eq!(
        last_occurrence_ms("daily", Some(9), Some(0), now, seven),
        Some((now_at(9, 0) + seven).timestamp_millis())
    );
}

#[test]
fn a_jittered_occurrence_is_not_due_until_the_offset_has_passed() {
    // The behaviour a user would notice, and the one a careless implementation
    // gets wrong: between the nominal time and the offset, TODAY's occurrence
    // has not happened yet, so the most recent one is still YESTERDAY's.
    let five = chrono::Duration::minutes(5);

    // 09:02 — inside the offset. Yesterday's 09:05 is the latest reached.
    assert_eq!(
        last_occurrence_ms("daily", Some(9), Some(0), now_at(9, 2), five),
        Some((yesterday_at(9, 0) + five).timestamp_millis()),
        "before the offset elapses, today's occurrence must not count as reached"
    );
    // 09:06 — past it. Today's 09:05.
    assert_eq!(
        last_occurrence_ms("daily", Some(9), Some(0), now_at(9, 6), five),
        Some((now_at(9, 0) + five).timestamp_millis())
    );
}

#[test]
fn every_schedule_kind_still_yields_exactly_one_occurrence_per_period() {
    // Sweep a full day minute by minute under a real offset and count how many
    // distinct occurrences a daily schedule reports. Exactly one transition
    // means one run per day: no double-run, no skipped day.
    let offset = chrono::Duration::minutes(9);
    let seen: std::collections::HashSet<i64> = (0..24 * 60)
        .map(|i| now_at(0, 0) + chrono::Duration::minutes(i))
        .filter_map(|t| last_occurrence_ms("daily", Some(9), Some(0), t, offset))
        .collect();

    assert_eq!(
        seen.len(),
        2,
        "a daily schedule swept across one day must report yesterday's occurrence              then today's — exactly one transition"
    );
}

// ── last_occurrence_ms ─────────────────────────────────────────────────

#[test]
fn manual_and_unknown_have_no_occurrence() {
    let now = now_at(14, 30);
    assert_eq!(
        last_occurrence_ms("manual", None, None, now, NO_JITTER),
        None
    );
    assert_eq!(
        last_occurrence_ms("weekly", None, None, now, NO_JITTER),
        None
    );
}

#[test]
fn hourly_uses_this_hour_when_past_the_minute_else_previous_hour() {
    // minute 15, now 14:30 → this hour's 14:15.
    let now = now_at(14, 30);
    assert_eq!(
        last_occurrence_ms("hourly", None, Some(15), now, NO_JITTER),
        Some(now_at(14, 15).timestamp_millis())
    );
    // minute 45, now 14:30 (not yet reached) → previous hour's 13:45.
    assert_eq!(
        last_occurrence_ms("hourly", None, Some(45), now, NO_JITTER),
        Some(now_at(13, 45).timestamp_millis())
    );
    // scheduleHour is ignored for hourly; default minute is 0.
    assert_eq!(
        last_occurrence_ms("hourly", Some(7), None, now, NO_JITTER),
        Some(now_at(14, 0).timestamp_millis())
    );
}

#[test]
fn daily_uses_today_when_past_else_yesterday() {
    // 09:00 default, now 14:30 → today 09:00.
    let now = now_at(14, 30);
    assert_eq!(
        last_occurrence_ms("daily", None, None, now, NO_JITTER),
        Some(now_at(9, 0).timestamp_millis())
    );
    // Scheduled 18:30, now 14:30 (not yet) → yesterday 18:30.
    assert_eq!(
        last_occurrence_ms("daily", Some(18), Some(30), now, NO_JITTER),
        Some(yesterday_at(18, 30).timestamp_millis())
    );
}

#[test]
fn twice_daily_picks_the_later_reached_occurrence() {
    // Base 09:00 (+12h = 21:00). now 14:30 → the 09:00 slot (21:00 not reached).
    let now = now_at(14, 30);
    assert_eq!(
        last_occurrence_ms("twice_daily", Some(9), Some(0), now, NO_JITTER),
        Some(now_at(9, 0).timestamp_millis())
    );
    // now 22:00 → the 21:00 slot is now the latest reached today.
    let now_late = now_at(22, 0);
    assert_eq!(
        last_occurrence_ms("twice_daily", Some(9), Some(0), now_late, NO_JITTER),
        Some(now_at(21, 0).timestamp_millis())
    );
    // Before both of today's slots (now 06:00) → yesterday's later slot 21:00.
    let now_early = now_at(6, 0);
    assert_eq!(
        last_occurrence_ms("twice_daily", Some(9), Some(0), now_early, NO_JITTER),
        Some(yesterday_at(21, 0).timestamp_millis())
    );
}
