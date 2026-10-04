use super::*;

// Pins these `pub(crate)` constants against hand-written literals — the TS
// mirror (`packages/shared/src/types/index.ts`, adjacent to `StatusEvent`)
// pins its own exported `EVENT_SOURCE_*` values against these SAME
// literals, since `pub(crate)` here means the renderer cannot import this
// constant directly and would otherwise silently re-declare its own copy
// (which is exactly what had happened before that TS-side fix). This value
// crosses that boundary with nothing else checking it: a rename on either
// side makes `isProvisionalEvent` (the renderer's `source`/`confirmed`
// check) stop matching, so a provisional email-derived row renders as
// settled history and the Accept/Reject affordance silently disappears —
// adjudication is the entire safety model that justifies auto-write
// defaulting ON, so losing it without a failing test is the worst
// available outcome. Compared against literals, not against each other or
// a derived value, so renaming both constants in lockstep still fails.
#[test]
fn event_source_constants_match_their_wire_literals() {
    for (name, actual, literal) in [
        ("EVENT_SOURCE_USER", EVENT_SOURCE_USER, "user"),
        ("EVENT_SOURCE_EMAIL", EVENT_SOURCE_EMAIL, "email"),
        (
            "EVENT_SOURCE_EMAIL_REJECT",
            EVENT_SOURCE_EMAIL_REJECT,
            "email_reject",
        ),
    ] {
        assert_eq!(actual, literal, "{name}");
    }
}
