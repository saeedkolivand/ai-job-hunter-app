# ADR-005: Universal thinking/reasoning normalization at the provider-adapter boundary

**Status:** Accepted

## Context

Different AI providers surface model reasoning in incompatible ways: OpenAI uses `reasoning_content` / `reasoning` fields on the stream delta, Gemini marks thought parts with `thought: true`, Ollama surfaces `message.thinking`, and local models (DeepSeek-R1, Qwen3, etc.) embed reasoning inline as `<think>…</think>` tags in the content stream. Surfaces that consume streaming output (AI Generate, analyze, the autopilot apply modal) each needed to handle these differences — or they would diverge.

## Decision

Every provider adapter maps its reasoning signal to the same `ai:stream` event shape — `{ delta, thinking: true }` — before emitting to the renderer. Inline `<think>…</think>` blocks (local models) are parsed by a shared stateful splitter (`apps/tauri/src/renderer/lib/generate/think-split.ts: createThinkSplitter`) that is called by the streaming surface before the delta is passed on, so the renderer has **zero per-provider branching**. Visible answer text goes to `onToken`; reasoning text goes to `onThinking`.

## Consequences

- Any new provider whose reasoning is structured (not inline tags) normalizes in its adapter; inline-tag models work via the shared splitter at the consumption point.
- All streaming surfaces automatically support multi-provider reasoning with no per-surface changes.
- `createThinkSplitter` is stateful across deltas; callers must create one instance per stream and call `flush()` at stream end.
- Adding a provider with a new reasoning signal = adapter change only, no renderer change.
