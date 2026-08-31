---
name: agent-cli-standards
description: Agent-facing CLI standards — the argv-sentinel binary mode, the thin-client-over-the-bridge rule, allowlist projections, prompt-fencing third-party text, the stable machine-readable output contract, exit codes, throttling, and destructive-command operability. Load for changes under extension_bridge/agent_cli.rs, agent_read.rs, and anything that adds or changes an agent CLI verb.
---

# Agent CLI standards

Standards for `ajh-tauri agent …` — the machine-facing control surface an AI agent drives instead of
reading pixels off a native window (issue #1084). Load with `author-contract` (authors) /
`token-efficiency` + `critic-contract` (reviewers). The bridge transport itself is
`extension-standards`; this skill is the CLI **surface** on top of it.

## The consumer is not a human

Output goes into an LLM's context. That single fact drives most of what follows: the output is a
contract, third-party text in it is an injection vector, and a path in an error message is a privacy
leak into a transcript that may be pasted into a bug report.

## Architecture (settled — do not redesign without an ADR)

- **A MODE of the existing binary, never a second `[[bin]]`.** The release upload globs only read
  `target/release/bundle/**` (`.github/workflows/release.yml`), so a second binary ships to nobody.
  The exe is already installed and already registered in the native-messaging manifests.
- **The argv sentinel's position is load-bearing in both directions** (`main.rs`):
  - BELOW the native-host short-circuit.
  - ABOVE `ajh_tauri::run()` — `run()` forks the minidump supervisor as its first act, and the
    single-instance plugin would otherwise hand the CLI's argv to the running GUI, pop its window,
    and exit having printed nothing.
- **A thin client over the loopback bridge — never a second reader of the stores.** The app must be
  running. Reading the data directory from a second process is not read-only: `ApplicationStore::open`
  runs `link_orphaned_generations` + `backfill_from_generations`, which write to `ai_generations.db`
  and can CREATE `Application` rows, and two processes racing `run_migrations` (which reads
  `user_version` outside any transaction) can leave the app booted with no store at all.
- **The app finds itself, the CLI does not guess.** `AJH_DATA_DIR` never escapes the app process and
  the AppHandle-free fallback is not the install location, so the app writes a pointer file carrying
  `exePath` (from `current_exe()`) and `dataDir` on every launch, on `register_native_host`'s existing
  best-effort/idempotent lifecycle. The binary is not on `PATH` on Windows, macOS, or Linux AppImage.

## Data leaving the app

- **Allowlist projections, never delegated records.** A resource returns a struct that _cannot express_
  the forbidden fields — absent by construction, not by remembering to omit. `Autopilot` alone carries
  `resume_text`, `cover_letter`, `assistant_notes`, `assistant_provider/model/base_url`, full
  `found_jobs` descriptions and `last_run_summaries`.
- **The guarantee stops at every field you did not re-declare.** A nested field typed as the _source_
  struct is a passthrough, and a forbidden-key test that matches a hardcoded name list plus an
  exact-keys test that only reads top-level keys are both blind to it. Project nested types too, and
  assert nested key sets.
- **Fence third-party text — and never let the fence carry a guarantee.** Scraped job descriptions are
  attacker-authorable and go to a consumer that may hold shell tools. Route them through
  `prompt_fence::fenced("job_posting", …, JOB_CAP)` — the same primitive, tag and cap
  `answer_assist.rs` uses on the identical string. But adaptive attacks defeat >90% of published
  prompt-injection defenses, so fencing is attenuation, not a gate: no design may depend on it holding.
  A payload can imitate a closing fence; it cannot as easily imitate a marker interleaved _throughout_
  the untrusted span, so prefer datamarking to edge delimiters when strengthening this.
- **Untrusted text supplies data, never control flow.** A job posting must never influence which verb
  runs, which id it targets, or whether a confirmation is satisfied — only fill fields the caller's
  plan already named. This is a structural rule and it survives the fence being bypassed.
- **The threat is live, not theoretical.** Roughly 1% of 200,000 real résumés carried prompt injections
  with a sevenfold rise over 16 months, and employers have begun seeding job postings with hidden
  instructions — the exact content this CLI scrapes and hands to a shell-capable agent.
- **Path privacy in every emitted string**, including usage errors — never echo raw argv, which can be
  a path containing a username.

## The output is a contract

- One JSON document on stdout per invocation. Exit `0` success · `1` the app refused (printed as JSON)
  · `2` the round trip never completed or usage was invalid · `4` the mutation needs confirmation.
- **A mutating verb returns a confirmation envelope, never an interactive prompt.** An autonomous
  caller cannot answer a `y/N`; faced with one it reaches for `--force`, which is strictly worse. So
  without confirmation a mutation exits `4` and prints what it _would_ change plus the exact command
  to re-run. Approval fatigue is a documented, neural effect — a dialog answered every time protects
  nobody, so vary the ceremony by blast radius rather than repeating one prompt.
- **Unknown verbs and flags are hard failures.** No fuzzy matching, no "did you mean", no prefix
  abbreviation, ever. An agent that can be nudged from a typo into a neighbouring command will
  eventually be nudged into a destructive one; the command either exists or it does not.
- **Distinct error sentinels for distinct causes.** Collapsing several real causes into one name is a
  defect, not a simplification — `app_not_running` for a merely-missing pointer sent this feature's own
  verification pass looking in the wrong place. Never encode an unsound distinction either: a crash
  between `challenge` and `auth` is not a pairing failure.
- **Nothing hand-maintained that can drift.** Help text and the resource/verb list are derived from the
  same table the dispatcher matches on, with a test asserting every listed verb parses and every
  parseable verb is listed. `--help` must work with the app **closed** — help that needs the thing you
  are trying to reach is useless.

## Availability + abuse

- **Throttles live on `BridgeState`, never per-connection.** Every CLI invocation is a fresh process and
  a fresh socket, so a per-connection bucket is bypassed by construction. Size buckets per verb; a
  compute-heavy verb gets its own instance rather than sharing a cheap-read bucket.
- **Bound the computation, not only the output.** A `limit` that truncates rows after an unbounded
  clustering pass bounds nothing.
- **Never block the connection read loop.** Spawn multi-second work and reply through `out_tx`
  (`stream::spawn_answer_assist` is the precedent) — an inline `await` also delays that connection's
  `token.revoked` observation.
- **Absolute deadlines, never re-armed ones.** A `timeout` re-armed inside a loop that skips frames is
  not a deadline; a peer sending pings faster than the budget hangs the call forever.
- **`try_state`, never `state::<T>()`, in a frame handler** — release is `panic = "abort"`.

## Windows

The release build is `windows_subsystem = "windows"`, so an interactive run has no console and
`println!` on an invalid stdout panics into a silent abort (no message, no crash report — the CLI
short-circuits above `crash_reporting::init`). Probe `GetStdHandle(STD_OUTPUT_HANDLE)` **first** and
leave a valid handle alone; an inherited pipe is the agent's case and the common one. Attaching
unconditionally replaces that pipe and breaks the primary consumer.

## Destructive commands

The owner has decided destructive and irreversible commands are **in scope** and must _work_ — do not
propose gating them away, and do not re-argue consent. The job is to make them safely **operable**.
Guard rails that exist only as a renderer `ConfirmModal` do not exist for a CLI caller; anything
relied upon must be reimplemented deliberately on the Rust side.

- **Every verb declares its own risk, and an undeclared verb is destructive.** Default pessimistic —
  read-only, idempotent and reversible are claims a verb must make, not assumptions callers may hold.
  The declaration belongs in the same table `schema` and `--help` are derived from.
- **Severity scales the confirmation, and the severe tier requires the resource's own name.** Typing
  the name is the one confirmation a hallucinated argument cannot satisfy, because it forces the caller
  to have actually read the record. `--confirm="<name>"` keeps that scriptable.
- **An empty variable must never widen a selector.** `--id "$X"` with `X` unset is the canonical
  catastrophe — it must be an error, never "all". Any selector that can expand to everything is
  treated as a destructive operation regardless of the verb.
- **Irreversible verbs take a caller-supplied idempotency key**, and a replay returns the stored first
  result — including a stored error. Model three states, not two: absent, in-flight, complete. Treating
  in-flight as absent is what turns a retry into a duplicate application or a double charge.
- **Plan and apply are separate artifacts.** A plan the agent narrates in prose is not a plan; write it
  to a file and refuse to apply it if the underlying state moved since it was produced.
- **A safety floor no allowlist can lift.** The truly unrecoverable targets (wipe app data, sign out
  everywhere, delete every application) sit below any allow rule or config, and the floor is checked
  _before_ any allowlist is consulted — an allowlist evaluated first silently defeats it.

Spend and rate limiting are charged **per caller**, not in a shared chokepoint, and `limits::Limiter`
is in-memory and per-process. Any new path that reaches an AI provider must charge it explicitly or it
silently uncaps the daily budget.

## Validate before done

`cargo fmt` · `cargo clippy --all-features --all-targets -- -D warnings` · `cargo test --lib` ·
`cargo test --test architecture`. Then **run the real binary** against a running app: a unit test
against the state machine is not evidence that the two halves talk. `cargo test --lib` flakes roughly
1 run in 3 with a pre-existing `rate_limiter` subtract-with-overflow panic in scraping tests — re-run
and say so rather than treating it as yours.
