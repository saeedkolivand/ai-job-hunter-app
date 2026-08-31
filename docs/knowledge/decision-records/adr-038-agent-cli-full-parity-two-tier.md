# ADR-038 — Full CLI parity: a policy table over all 164 commands, curated and generic tiers kept apart

**Status:** Accepted

**Date:** 2026-08-31

**Deciders:** repo owner, main session

## Context

[ADR-037](adr-037-agent-cli-as-binary-mode-thin-client.md) shipped five hand-written, projected read
Resources. The owner then asked for `gh`-style parity: the CLI should perform **any** action the user
can perform in the UI, **including** irreversible ones (`privacy:reset_app`, `sign_out_all`,
`credentials:*`, the `*_remove` family, and application submission when it exists), with the dispatcher
**derived** from the registry so a new capability needs no CLI work.

Three facts, each independently reproduced, shaped the design:

- There are **164** `#[tauri::command]` sites, exactly 1:1 with `generate_handler!` (`lib.rs`), diffed
  both directions.
- **`IPC_CHANNELS` is not that registry.** `NOTIFICATIONS_CHANNELS` is literally `{} as const` and
  `AI_CHANNELS` has 5 entries for a 29-method contract, so deriving from it ships a CLI that silently
  cannot do what the UI does.
- **`Webview::on_message(InvokeRequest, responder)` is `pub`** in tauri 2.11.5 (verified in the vendored
  pinned source), so invoking a command by name from Rust needs **no** codegen.

Four commands have **zero** renderer references, one of them destructive — so registry parity gives the
CLI _more_ than the UI, not the same. That is accepted and is why the policy table is explicit.

## Decision

**1. A committed policy table is the allowlist, and it must match the registry exactly.** Every one of
the 164 commands carries a declared `Effect` — `Read`, `Reversible`, `Irreversible`, or `NotExposed`
with a stated reason — and a test asserts the table and `generate_handler!` agree with no extras and no
missing entries. This is [ADR-014](adr-014-cli-agent-shell-plugin-static-allowlist.md)'s static-allowlist
invariant applied to _inbound_ dispatch: an unclassified command fails CI instead of shipping. Effects
are declared, never inferred, and pessimistic by default.

**2. Two tiers with visibly different grammars.** `agent <resource>` is curated and projected;
`agent call <ns>:<command>` is generic. AWS ships exactly this split (`s3` vs `s3api`) and documents
that the curated tier loses the generated tier's machinery. Keeping the grammars distinct means the
caller knows _before running_ whether the reply carries a projection guarantee.

**3. The generic tier returns the record raw.** No PII redaction — see the amendment below.
Third-party scraped text is still fenced: that protects the agent from an attacker-authored posting,
not the user from their own résumé, and the two are different concerns.

**4. An irreversible command demands a proof the refusal does not contain.** There is no derivable
dry-run — no simulation path exists anywhere in the app — so the safety property comes from _where the
token lives_: the refusal names which Resource yields it and withholds the value, so satisfying it
requires a second call to a different Verb, and therefore requires having actually read the record.
A one-hop ceremony that hands back its own answer stops nothing.

**5. `ok` is not overloaded in the generic tier.** ~47 commands signal failure in-band inside a `Value`
rather than as `Err`, so the dispatcher cannot know whether a call succeeded. It reports `dispatched`
and returns the payload verbatim; the curated tier keeps a truthful `ok` because those five are
hand-written. The alternative — a per-command failure-detection column derived from doc comments — is
exactly the drift-prone hand-maintained literal AGENTS.md rule 17 forbids.

## Consequences

### Positive

- **Spend limits survive for free.** `limiter.acquire` and `charge_provider_daily` sit _inside_ the
  command bodies (`commands/ai/mod.rs`), and `on_message` invokes the real command in the app's own
  process against its single managed `Limiter`. The guard that exists so "a looping/XSS'd renderer
  can't drive unbounded paid-API spend" catches a looping agent too, unchanged.
- **A new capability appears with no CLI work**, and a new _unclassified_ one fails CI.
- No codegen, no build-step model, no second source of truth.

### Tradeoffs

- **The policy table is 164 rows of one-time manual classification.** That tedium is the point: it is
  the enumerated allowlist.
- **Parity exceeds the UI.** Four commands the UI never calls become reachable, one destructive.
- **No preview.** Irreversible commands cannot be simulated; the ceremony is the only control.

### Amends ADR-0005 (network egress privacy boundary)

[ADR-0005](0005-network-egress-privacy-boundary.md)'s network-egress privacy boundary is hereby **scoped to the
curated tier**. `agent call` returns records raw, including `resume_text` and `cover_letter`, into a
consumer that is by design an LLM context. This was put to the owner with the consequence stated and
chosen deliberately. It is recorded here rather than left implicit, because a promise that quietly
became false is worse than one that was openly narrowed.
