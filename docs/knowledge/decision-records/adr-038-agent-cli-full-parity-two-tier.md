# ADR-038 — Full CLI parity: a policy table over every registered command, curated and generic tiers kept apart

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

- Every `#[tauri::command]` site is exactly 1:1 with `generate_handler!` (`lib.rs`), diffed both
  directions — the current count is pinned by `policy::tests::policy_table_row_count_is_pinned`, never
  restated here since it moves with every new command.
- **`IPC_CHANNELS` is not that registry.** `NOTIFICATIONS_CHANNELS` is literally `{} as const` and
  `AI_CHANNELS` has 5 entries for a 29-method contract, so deriving from it ships a CLI that silently
  cannot do what the UI does.
- **`Webview::on_message(InvokeRequest, responder)` is `pub`** in tauri 2.11.5 (verified in the vendored
  pinned source), so invoking a command by name from Rust needs **no** codegen.

Four commands have **zero** renderer references, one of them destructive — so registry parity gives the
CLI _more_ than the UI, not the same. That is accepted and is why the policy table is explicit.

## Decision

**1. A committed policy table is the allowlist, and it must match the registry exactly.** Every
registered command (row count pinned by `policy::tests::policy_table_row_count_is_pinned`) carries a
declared `Effect` — `Read`, `Reversible`, `Irreversible`, or `NotExposed`
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

- **The policy table is one row per command of one-time manual classification** (row count pinned by
  `policy::tests::policy_table_row_count_is_pinned`). That tedium is the point: it is the enumerated
  allowlist.
- **Parity exceeds the UI.** Four commands the UI never calls become reachable, one destructive.
- **No preview.** Irreversible commands cannot be simulated; the ceremony is the only control.

### Amends ADR-0005 (network egress privacy boundary)

[ADR-0005](0005-network-egress-privacy-boundary.md)'s network-egress privacy boundary is hereby **scoped to the
curated tier**. `agent call` returns records raw, including `resume_text` and `cover_letter`, into a
consumer that is by design an LLM context. This was put to the owner with the consequence stated and
chosen deliberately. It is recorded here rather than left implicit, because a promise that quietly
became false is worse than one that was openly narrowed.

## Amendment — 2026-09-07

§2 and §3 say the generic tier hands the record back as the command produced it. That still holds for
**content**: no projection, no redaction, no renaming — `agent call` returns the command's own value.
It does not extend to **transport**. Two replies cannot survive the trip intact, and for those the
agent layer may reshape the envelope — for these two enumerated reasons and no others:

- **A reply too large to cross the bridge.** A frame over `extension_bridge::MAX_FRAME_BYTES` is closed
  without being parsed, so the caller saw `connection_lost`: a transport sentinel standing in for a size
  refusal, indistinguishable from a dropped socket and inviting a retry that can never succeed. The
  dispatcher now measures the serialized reply against that same cap — reusing the bridge's constant
  rather than defining a second ceiling — and substitutes a deterministic `result_too_large` refusal.
  That substitute fits by construction, not by being merely smaller than what it replaced: it is
  assembled only from bounded material — a fixed sentinel, a detail whose one variable is the measured
  size, and identifiers clamped before they are echoed back, since they arrive from the caller and
  nothing else would bound them (`enforce_frame_cap` and the clamp beside it in `agent_call.rs`).
  Same sentinel as [ADR-040](adr-040-mcp-server-as-agent-cli-mode.md) §10, which caps the MCP
  tool result one hop further out against its own, smaller constant; adopting that one here would newly
  refuse payloads a plain `agent call` caller receives today, so the two stay separate.
- **Rows and bytes no command argument can narrow.** Where no parameter bounds a command's result — the
  whole-table list commands — the generic tier pages it behind `limit`/`cursor` and returns an
  `items`/`total`/`nextCursor` envelope: the paging discipline the curated `found-jobs` resource
  introduced (#1115), under a generic row key because the generic tier has no per-resource name to use.
  Where the value is raw bytes, JSON _can_ carry them — as an array of numbers — but only at several
  times their own size in decimal digits and separators, enough for an ordinary export to overrun the
  MCP result cap with no argument available to narrow it. So the agent layer base64-encodes those
  bytes and marks the encoding explicitly on the reply, and a caller decodes on the marker instead of
  inferring it from the bytes (`base64_byte_fields` in `agent_call/reshape.rs`; its audited `(command, field)`
  pairs carry the measurement that sized the choice).

The page size is an audited constant beside the generic tier's reshaping (`agent_call/reshape.rs`), carrying its derivation
on the constant itself; the size ceiling is the bridge's own `MAX_FRAME_BYTES`, reused rather than
copied. That placement is the control: each value is reviewable in one place, and a row that starts
paging is a change to a line rather than to scattered call sites. The offset/limit/byte-budget
primitives themselves are one module (`extension_bridge/paging.rs`). The limit clamp and the byte
budget are shared with the curated `found-jobs` resource; the cursor GRAMMAR is not, and deliberately
so — that module owns the rule (an unreadable cursor refuses, it never silently resets to page 1),
while each surface keeps its own vocabulary, which is what let `found-jobs` scope its cursor to the
autopilot that issued it without this tier changing at all.

**Rejected — teach the shared commands optional `limit`/`cursor` and an encoding flag.** That would keep
the agent layer a pure pass-through, but those commands are the ones the renderer calls through
`AppClient`, so the wire shape a UI screen already consumes would move to serve a caller that is not the
UI. Confining the reshape to the agent layer cannot regress the renderer at all. Same reasoning as §5,
which keeps the generic tier's own `dispatched` vocabulary inside the dispatcher rather than teaching
every command a new one.
