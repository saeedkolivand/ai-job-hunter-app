//! The quality floor for the two JSON stages (`analyze_job`, `strategy`), #1382.
//!
//! A stage sends the cheapest effort tier by default. On a thinking-capable
//! local model that tier can answer with a structurally valid but empty
//! artifact, and the stage cache then serves it for a week. So a floor miss is
//! retried ONCE at the provider's default (no effort sent), the better of the
//! two answers is kept, and a result that still misses the floor is never
//! written to the cache.

use std::future::Future;

use super::cache::{self, StageCacheKey};
use crate::error::AppResult;
use crate::pipeline::cache::KvCache;

pub(crate) struct Floored<T> {
    pub value: T,
    /// Still below the floor after the retry (or with nothing to retry).
    pub degraded: bool,
    /// A retry was ATTEMPTED (a second call was started). It may never have
    /// reached a provider: the deadline guard can refuse it before the request.
    pub retried: bool,
}

/// Ask once at `effort`; on a floor miss ask once more at the default.
///
/// No retry when `effort` is already `None`: the second request would be the
/// first. A failing retry keeps the first answer, since the caller can still
/// use it and the deadline state is checked at the next stage boundary.
///
/// **Cancellation:** both JSON stages opt into `Stage::abandon_on_cancel`, so a
/// cancel drops this whole future, in-flight call and retry included. The
/// deadline is honoured too: the retry goes through `complete_json`, whose guard
/// refuses a call past the run deadline.
pub(crate) async fn with_floor<'e, T, F, Fut>(
    effort: Option<&'e str>,
    mut ask: F,
    degraded: impl Fn(&T) -> bool,
    richness: impl Fn(&T) -> usize,
) -> AppResult<Floored<T>>
where
    F: FnMut(Option<&'e str>) -> Fut,
    Fut: Future<Output = AppResult<T>>,
{
    let first = ask(effort).await?;
    if !degraded(&first) || effort.is_none() {
        let degraded = degraded(&first);
        return Ok(Floored {
            value: first,
            degraded,
            retried: false,
        });
    }
    let value = match ask(None).await {
        Ok(second) if !degraded(&second) || richness(&second) > richness(&first) => second,
        _ => first,
    };
    let degraded = degraded(&value);
    Ok(Floored {
        value,
        degraded,
        retried: true,
    })
}

/// Write a stage artifact to the cache unless `skip` (a cache hit, or a floor
/// miss — see the module doc). The one place that rule lives.
pub(crate) fn store_sound(
    cache: Option<&KvCache>,
    stage: &str,
    key: &StageCacheKey,
    artifact_json: &str,
    skip: bool,
) {
    if !skip {
        cache::put(cache, stage, key, artifact_json);
    }
}

#[cfg(test)]
mod tests;
