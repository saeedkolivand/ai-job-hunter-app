---
name: code-quality
description: Shared clean-code / DRY / KISS / YAGNI standards and stack-specific smells for the React 19 + TypeScript + Rust (Tauri 2) monorepo. Library consumed by the code-quality-reviewer and code-quality-author agents.
disable-model-invocation: true
---

# Code Quality Standards

Standards for a pnpm/Turbo monorepo: React 19 + TypeScript frontend, Rust (Tauri 2) backend. The reviewer reports violations against this; the author fixes against it. Both apply the judgment below — these are heuristics, not a linter.

## Severity

- **High** — bugs in waiting: real logic duplication that will drift; a function doing >1 job over shared mutable state; `any`/unchecked casts hiding type holes; `unwrap()`/`expect()`/`panic!` on fallible paths in non-test Rust; swallowed errors.
- **Medium** — friction: functions >~50 lines or nesting >3; unclear names; magic numbers/strings; prop drilling; repeated literals; dead params.
- **Low** — polish: comment-as-deodorant, inconsistent ordering, minor naming, anything the formatter already owns.

## Principles — and where they backfire (do NOT over-apply)

- **DRY**: dedupe _knowledge_, not coincidental similarity. Two blocks that look alike but change for different reasons stay separate — a wrong abstraction costs more than duplication. Don't unify before the third occurrence, and only when the rule is truly the same.
- **KISS / YAGNI**: prefer the boring direct solution. Strip speculative generality — flags, params, hooks, and layers with one caller. Never add abstraction "for later."
- **Single responsibility**, not dogmatic SOLID. Split when there are two reasons to change, not to hit a line count.
- Net result must be _simpler_. A change that adds indirection, interfaces, or files without removing real complexity is a regression — record it as "considered, rejected," don't ship it.

## Stack smells

React/TS:

- A component doing fetch + state + formatting + render → extract a hook / presentational split.
- State derivable from props or other state (compute, don't store); effects syncing state that should be derived.
- Over-memoization (`useMemo`/`useCallback` with no measured need) as much as missing memo on a hot path.
- `any`, `as` assertions, stringly-typed unions → discriminated unions / generics. Enum where a union literal is simpler. Non-null `!` masking a real nullable; `// @ts-ignore`.
- Prop drilling >2 levels → context or composition. Exported types/components with no consumer.
  Rust/Tauri:
- `unwrap`/`expect`/`panic!` on I/O, parse, lock, or command paths → `Result` + `?` + a real error enum (thiserror).
- Clone-happy code where a borrow works; `String` params that should be `&str`.
- Fat command handlers → push logic into testable fns; keep `#[tauri::command]` thin. `unsafe` without a justifying comment.
  Cross-cutting: duplicated literals/config → one const/module; magic numbers → named; deep nesting → guard clauses / early returns; a boolean param gating two behaviors → split the function.

## External standards & best-practices (verified 2026-06-19)

These are **tensions to manage, not rules to obey**.

- **SOLID** — SRP/OCP/LSP/ISP/DIP guide module boundaries; apply where they cut coupling, not reflexively. https://martinfowler.com/bliki/
- **DRY** — dedupe _knowledge_, not coincidentally-similar code; premature DRY couples unrelated call sites. https://martinfowler.com/bliki/BeckDesignRules.html
- **KISS / YAGNI** — simplest thing that works; DRY routinely **collides with KISS** → prefer the clearer code; add abstraction at the _second_ real use (rule of three). https://martinfowler.com/bliki/Yagni.html
- **AHA (Avoid Hasty Abstractions)** — the current critique of over-abstraction: a wrong abstraction costs more than duplication; inline a leaky abstraction back before re-splitting. https://kentcdodds.com/blog/aha-programming · https://sandimetz.com/blog/2016/1/20/the-wrong-abstraction
- Smallest behavior-preserving diff; **name the trade-off** (coupling↔reuse, simplicity↔flexibility) rather than citing an acronym.

**Common mistakes:** citing DRY to justify a shared abstraction over two superficially-similar but semantically-unrelated blocks (premature coupling); treating SOLID/YAGNI as pass/fail gates → ceremony (interfaces with one impl, factories for one type).
