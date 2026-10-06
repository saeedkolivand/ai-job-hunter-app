/**
 * Browser-extension ⇄ desktop WebSocket bridge protocol (Feature 2).
 *
 * The single source of truth for the local WS frame format. The Rust side
 * (`apps/desktop/src-tauri/src/extension_bridge`) mirrors these string literals;
 * a parity test on the Rust side pins its message-type constants to the
 * `EXTENSION_MESSAGE_TYPES` values here so the two can never drift.
 *
 * The wire-message constants, the handshake message canonicalization, and the
 * payload TYPES live in the zod-free `./extension-protocol-constants.ts` (so the
 * browser extension can import them without pulling zod into its bundle). The
 * zod SCHEMAS live in `./extension-protocol/<group>.ts`, each bound to its
 * constants-file interface via `satisfies z.ZodType<…>` so the schema and its
 * type can never drift. This module re-exports the constants/types and every
 * schema so it remains the single barrel entry for desktop/renderer consumers.
 * Pure data + Zod — no `window`, no Node.
 */

export * from './extension-protocol/answers.js';
export * from './extension-protocol/document-export.js';
export * from './extension-protocol/envelope.js';
export * from './extension-protocol/match-agent.js';
export * from './extension-protocol/settings.js';
export * from './extension-protocol/tracking.js';
export * from './extension-protocol-constants.js';
