//! Agentic controller foundation (Phase 1 — backend only).
//!
//! A budgeted, cancellable tool-calling loop over the centralized AI provider
//! layer. This is a **human-in-the-loop assistant**: it is read-heavy, and any
//! write/spend action is DENIED in Phase 1 (a Phase-3 seam requires explicit user
//! confirmation before a write ever executes).
//!
//! SECURITY INVARIANT (prompt-injection defense — OWASP LLM01): the system prompt
//! is fixed and trusted. Scraped job text, résumé text, the user's question, and —
//! critically — tool RESULTS are untrusted DATA. They ride only in `User`/`Tool`
//! transcript turns (fenced), and are NEVER merged into the system prompt or a
//! tool description. The controller in [`controller`] enforces this; the registry
//! in [`tools`] holds only fixed, trusted schemas (least privilege, LLM06).
//!
//! Nothing here is wired to a Tauri command yet — Phase 2 adds `agent_run`.

pub mod controller;
pub mod tools;
