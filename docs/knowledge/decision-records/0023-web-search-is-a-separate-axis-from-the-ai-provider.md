---
status: accepted
---

# Web Search Is a Separate Axis From the AI Provider

## Context

Company research was defined as "the active AI provider's **own** web search". That
reads as a clean local-first constraint — one vendor, the one the user already
chose — but it silently excluded a large share of users:

- **`openai-compatible` gateways** report `supports_web_search: false`. Every LM
  Studio, vLLM, OpenRouter, Groq, Together and DeepSeek user got no research at
  all.
- **Local Ollama** reports `supports_web_search: true` for the _family_, but the
  Ollama Web Search API needs a separate **ollama.com account key**. A keyless
  local install therefore showed the "search company" toggle **ON** and returned
  an empty brief on every single generation.

The second case is worse than the first, because nothing surfaced it. A
2026-08-08 diagnostics bundle shows the consequence: six cover letters written
with no company knowledge, indistinguishable from six written with a full brief,
and 28 lifetime `no usable brief (provider found nothing)` lines that no user
would ever see.

The code was already shaped for the fix. `research.rs` is provider-agnostic (the
`SearchResult` shape, the query builders, the synthesis prompts, the
prompt-injection guard), and the three Ollama research functions were each
"search → if empty return `""` → synthesize with the provider's own model". Only
the **search step** was ever provider-specific.

## Decision

**Web search is a separate axis from generation.** A _search backend_ returns web
results; an _AI provider_ generates text. They are configured independently, and
a search backend deliberately does not appear in `ProviderId`, in
`Completer::from_active`'s routing, or in the renderer's provider registry — it
cannot generate, and modelling it as a provider would offer it as one.

The search step sits behind a `WebSearcher` trait with two implementations today:
the Ollama Web Search API, and **Exa** (`api.exa.ai/search`) as a
user-configurable fallback keyed at `ai:exa`.

Three sub-decisions, each of which had a real alternative:

1. **Fallback only, resolved from configuration before any call.** A provider
   that can already search keeps using its own search, _even when an Exa key is
   stored_. Rejected: preferring Exa whenever a key exists — likely better
   retrieval, but it silently moves spend to a second vendor and changes
   behaviour for users whose research already works. The resolution is a pure
   predicate (`resolve_search_backend`) so the rule is pinned by a test rather
   than by convention.

2. **No second chance.** A native search that runs and returns nothing is _not_
   retried against the fallback. Rejected: retrying — it would rescue the 28
   observed empty-brief cases, but costs a second billable call on every native
   miss and lets two vendors see the same query in one pass.

3. **Synthesis stays on the user's own model.** Exa's own answer endpoint would
   collapse search+synthesis into one call, but it would bypass both
   `research::SYNTH_SYSTEM`'s prompt-injection guard — search results are
   attacker-reachable text, and this is the one stage that reads them unfenced —
   and the `is_no_info` filter, while moving generation spend to a second vendor.

The fallback lands on the `AiProvider::research` **default**, so providers whose
model searches for itself override it and are untouched, and a new provider with
no search inherits the fallback with no per-provider change.

**`supportsWebSearch` now answers a configuration question, not a capability
one:** true when research can actually run. This is a deliberate behaviour change
— a keyless Ollama user who saw the toggle ON will now see it OFF, which is the
truth they were previously denied.

## Egress

No change to [ADR 0005](0005-network-egress-privacy-boundary.md). Exa is a new
_vendor_ inside the already-enumerated **class 3 (web search, opt-in)**, not a new
egress class, and it is opt-in by construction: nothing is sent until the user
stores a key. It receives exactly what the current web search already receives —
a company name, a role title, a question topic — never résumés, generations,
applications or credentials. The call is Rust-side through `net::http`, so no
renderer CSP entry is involved.

README.md and SECURITY.md enumerate the egress classes and must name Exa as an
option within the web-search class.

## Addendum 2026-11-11 (#1500): failures are reported, not swallowed

`try_search` is now the required `WebSearcher` method and returns a typed error; `search` is a provided wrapper that degrades it to empty. The "search → if empty return `""`" shape above still holds for a search that found nothing, but a search that failed (billing, auth, rate limit) is an `Err` that research reports as a failed status instead of reading as an empty brief. See `commands/ai_provider/search/mod.rs` and `cover_letter/research/mod.rs`.

## Consequences

- Research works for every provider, including ones that never had it.
- Exa is billed per request by a different vendor than the AI provider, so it
  charges its own `(day, vendor)` bucket rather than spending the provider's.
- The capability query now depends on stored keys, so the key mutations must
  invalidate it — its `VERY_LONG` staleTime would otherwise strand the toggle
  until restart.
- Adding a second search backend later is a `WebSearcher` impl plus a credential
  slot. If a third appears, the two-arm `SearchBackend` enum should become a
  registry, in the shape `SCRAPERS` already uses.
- **Not adopted:** surfacing Exa's `costDollars` (returned per response) into the
  spend store. The store is token-shaped; a real per-search cost is a genuine
  improvement and a separate change.

## Addendum (2026-11-11): ordered backends, billing/auth fall-through, SearXNG-compatible server

Issue #1515. The axis split above stands; three things change.

**Order.** The provider's own search, then a SearXNG-compatible server, then Exa
(`search_order` / `SearchChain` in `commands/ai_provider/search/mod.rs`). SearXNG
sits before Exa because it is free and user-run. It is one backend for SearXNG
and RustXNG alike (same `/search?format=json` shape), with a user-set base URL and
no key; the URL is validated on save and again on read, and lives in
`AiConfigStore`, not the keychain. Agents may read it (`ai_search_settings`) but
not set it.

**"No second chance" now means EMPTY results only.** A backend that runs and
returns nothing is still never retried elsewhere. A native search or Exa that
fails with a billing or auth `FailureKind` does fall through to the next
configured backend, once each (`falls_through`): the call produced nothing and
the user's configuration is what is broken, so trying the next backend does not
double-bill or re-send a query that already succeeded. Rate-limited and
unavailable failures stay terminal.

**A SearXNG-compatible server's failures never fall through.** It is keyless and
user-run, so a billing/auth-shaped error from it is not a vendor account problem;
a 403 typically means the instance has JSON output disabled. Falling through
would hide that misconfiguration and move the query to a paid vendor the user
did not choose for that failure. It reports a typed error instead.

### Egress (addendum)

Still [ADR 0005](0005-network-egress-privacy-boundary.md) class 3 (web search,
opt-in); no new class. A SearXNG-compatible query sends the same company name,
role title or question topic as the other backends, as a GET query string to the
host the user typed, and nothing else (no résumé, generation or credential). That
host is user-chosen, so the static inventory in `tests/egress.rs` cannot list it. Plain
`http` is accepted only for local hosts;
credentials in the URL are refused. Nothing is sent until the user sets a URL.

Release note: the `ai_config` migration that stores the URL bumps the sync schema
version, so an older peer holds the newer payload until it is upgraded (see
[Schema versions](../sync-protocol.md#schema-versions)).
