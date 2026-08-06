/**
 * The stable, non-sensitive class of an unknown thrown value, for diagnostic
 * logging.
 *
 * Renderer `console.*` is forwarded into the rotated log file by
 * `src/log-bridge.ts`, and that file ships inside the diagnostics bundle a user
 * attaches to a bug report. So a caught error's `message` must never be logged
 * verbatim: the backend's export validation deliberately embeds the offending
 * URL in its message (`validate/mod.rs`'s `header_url_mismatch`), and a
 * résumé's header links are personal data. The Rust side already made exactly
 * this trade in `export/commands/mod.rs` — it logs the critical issue CODES,
 * never `issue.message` — so the "why" is recoverable from the same log file
 * without the renderer restating it.
 *
 * The user still sees the full message: `notify.error` renders it on screen,
 * where it is not persisted anywhere.
 */
export function errorClass(err: unknown): string {
  if (err instanceof Error) return err.name || 'Error';
  return typeof err;
}
