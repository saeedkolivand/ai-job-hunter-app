//! Agentic controller foundation.
//!
//! A budgeted, cancellable tool-calling loop over the centralized AI provider
//! layer. This is a **human-in-the-loop assistant**: it is read-heavy, and any
//! write/spend action SUSPENDS the run for explicit user confirmation before it
//! ever executes (the Phase-3 confirm gate in [`gate`]).
//!
//! SECURITY INVARIANT (prompt-injection defense — OWASP LLM01): the system prompt
//! is fixed and trusted. Scraped job text, résumé text, the user's question, and —
//! critically — tool RESULTS are untrusted DATA. They ride only in `User`/`Tool`
//! transcript turns (fenced), and are NEVER merged into the system prompt or a
//! tool description. The controller in [`controller`] enforces this; the registry
//! in [`tools`] holds only fixed, trusted schemas (least privilege, LLM06).
//!
//! SECURITY INVARIANT (excessive agency — OWASP LLM06): a [`tools::ToolKind::Write`]
//! tool never runs on the model's say-so. The controller suspends on the [`gate`]
//! (`AgentGate`) and executes only after a real `agent_confirm` decision — Deny,
//! timeout, and cancel all default to NOT acting. See [`gate`] for the full
//! confirm-gate invariants.
//!
//! The `agent_run` Tauri command drives the "prep this application" flow;
//! `agent_confirm` resolves its suspended Write confirmations.

pub mod controller;
pub mod flows;
pub mod gate;
pub mod tools;
