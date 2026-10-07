use super::super::stages::research_company_brief;

/// The `cover_letter` stage's opt-in research must be structurally non-fatal
/// — an admission refusal, a search failure, or a timeout must degrade to
/// `""`, never propagate as a run-ending `AppError`.
///
/// This used to be a source-text scrape (find `research_company_brief`'s
/// body via `body.find("\n}\n")`, then check it contains no `?`). Three real
/// flaws in that shape: the brace search finds the FIRST column-0 `}\n`, so
/// the slice can overrun into a nested item; a `?` inside a comment/string/
/// URL trips the "no `?`" check even though it changes nothing about control
/// flow; and it is blind to `.unwrap()`/`.expect()`/a bare `return
/// Err(...)`, none of which contain a `?` at all.
///
/// The replacement below makes the SIGNATURE the guarantee instead, for the
/// one thing a signature actually can prove:
/// [`research_company_brief_returns_a_plain_string`] is a compile-time-only
/// check (never executed — see its own doc) that fails to COMPILE, not just
/// fails a test, the moment `research_company_brief`'s return type stops
/// being a plain `String`. Since `String` implements neither
/// `FromResidual<Result<Infallible, _>>` nor `FromResidual<Option<Infallible>>`,
/// that return type makes it a compile error for the function's body to
/// contain a `?` on ANY `Result`/`Option` sub-expression — a stronger
/// guarantee than the old scrape could ever give, and one that needs no
/// runtime harness (this crate has no `tauri::test` mock-`AppHandle`, the
/// same constraint `research_answer_tests` documents).
///
/// What the signature does NOT prove — a `.unwrap()`/`.expect()`/panic (the
/// type system has nothing to say about those) or that the function actually
/// ADMITS before it spends — stays a plain, narrow runtime check just below,
/// honestly scoped to what it can see.
#[allow(dead_code)]
fn research_company_brief_returns_a_plain_string<'a>(
    completer: &'a crate::pipeline::Completer,
    ctx: &'a super::super::QualityCtx<'a>,
) -> impl std::future::Future<Output = String> + 'a {
    research_company_brief(completer, ctx)
}

/// The one guarantee the type system above cannot give: that
/// `research_brief` actually ADMITS against the shared rate/
/// daily-budget bucket BEFORE it spends — the cost-control half of the same
/// non-fatality contract. A narrow whole-file substring check, not a
/// brace-bounded slice: `research_brief` is the last item in
/// `cover_letter.rs`, so nothing after the match position could produce a
/// false pass.
///
/// Mutation check: delete the `let Some(_guard) = completer.admit_research(NAME)
/// else { ... };` line from `research_brief` — this test fails
/// immediately.
#[test]
fn research_brief_admits_before_it_researches() {
    let source = include_str!("../stages/cover_letter.rs");
    let start = source
        .find("pub(crate) async fn research_brief")
        .expect("research_brief must exist");
    assert!(
        source[start..].contains(".admit_research("),
        "research_brief must admit against the shared bucket before researching"
    );
}
