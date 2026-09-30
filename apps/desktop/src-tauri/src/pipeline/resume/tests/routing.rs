use super::super::cache::{StageCacheKey, StageIdentity};
use super::super::{pick, stage_cache_key_for, QUALITY_STAGES};
use super::support::id;

/// The never-silent-switch rule, in one assertion per branch.
///
/// Mutation check (executed): make `pick` return `map.values().next()` when the
/// stage is absent and the "every other stage" arm fails; make it ignore the
/// map entirely and the first arm fails.
#[test]
fn a_stage_uses_its_override_and_every_other_stage_uses_the_default() {
    let default = "default-model".to_string();
    let mut overrides = std::collections::HashMap::new();
    overrides.insert("strategy".to_string(), "big-model".to_string());

    assert_eq!(pick(Some(&overrides), &default, "strategy"), "big-model");
    for stage in QUALITY_STAGES.iter().filter(|s| **s != "strategy") {
        assert_eq!(
            pick(Some(&overrides), &default, stage),
            "default-model",
            "{stage} was never overridden and must not be switched",
        );
    }
    // No map at all — every test and every override-free run — is the same
    // thing as an empty one: nothing changes.
    assert_eq!(pick(None, &default, "strategy"), "default-model");
    assert_eq!(
        pick(
            Some(&std::collections::HashMap::new()),
            &default,
            "strategy"
        ),
        "default-model"
    );
}

/// The BINDING, not just its two halves: the key a stage gets is derived from
/// the routing THAT stage resolved to.
///
/// The coupling this exists for: `StageCacheKey` used to be seeded once from
/// the run's single completer, so an overridden stage would have read and
/// written the DEFAULT model's cache entry — serving one model's analysis to a
/// run that asked for another's, invisibly (a cache hit looks like a fast run).
/// Two configs differing in exactly one stage's override must therefore differ
/// in exactly that stage's key.
///
/// Mutation check (executed): change `stage_cache_key_for` to
/// `base.rebound(identity(default))` — i.e. the default instead of the picked
/// routing, the exact mutation that used to stay green — and the
/// overridden-stage assertion fails.
#[test]
fn the_stage_cache_key_binding_follows_the_override() {
    let default = id("ollama", "default-model", None);
    let base = StageCacheKey::new(default, "seed");
    let plain: std::collections::HashMap<String, StageIdentity<'static>> =
        std::collections::HashMap::new();
    let mut overridden = std::collections::HashMap::new();
    overridden.insert("strategy".to_string(), id("ollama", "big-model", None));

    for stage in QUALITY_STAGES {
        let key_of = |map: &std::collections::HashMap<String, StageIdentity<'static>>| {
            stage_cache_key_for(&base, Some(map), &default, stage, |i| *i).key()
        };
        if *stage == "strategy" {
            assert_ne!(
                key_of(&plain),
                key_of(&overridden),
                "the overridden stage must not reuse the default model's entry",
            );
        } else {
            assert_eq!(
                key_of(&plain),
                key_of(&overridden),
                "{stage} did not change routing, so its cached artifact is still valid",
            );
        }
    }
}

/// The same binding over the CONTEXT-WINDOW axis: an override that changes only
/// the window still has to move only its own stage's key.
///
/// Mutation check (executed): remove `{window}` from `key`'s pre-hash, or make
/// `stage_cache_key_for` use the default instead of the picked routing, and
/// this fails. NOT covered (see the residue list above): `StageIdentity::of`
/// itself returning the wrong window — that read needs a `Completer`.
#[test]
fn a_window_only_override_moves_only_that_stages_cache_key() {
    let default = id("ollama", "m", Some(4_096));
    let base = StageCacheKey::new(default, "seed");
    let mut overridden = std::collections::HashMap::new();
    overridden.insert("draft".to_string(), id("ollama", "m", Some(32_768)));

    let key_of =
        |stage: &str| stage_cache_key_for(&base, Some(&overridden), &default, stage, |i| *i).key();
    assert_ne!(key_of("draft"), key_of("strategy"));
    assert_eq!(key_of("strategy"), base.rebound(default).key());
}

/// The provider half of the identity counts too — the same model name served by
/// a different provider is a different function.
///
/// Mutation check: drop `provider` from `rebound`'s output and this fails.
#[test]
fn rebinding_the_provider_changes_the_key_and_rebinding_to_itself_does_not() {
    let base = StageCacheKey::new(id("ollama", "m", None), "seed");
    assert_ne!(base.rebound(id("openai", "m", None)).key(), base.key());
    assert_eq!(
        base.rebound(id("ollama", "m", None)).key(),
        base.key(),
        "an override-free run must hit exactly the entries it always did",
    );
}
