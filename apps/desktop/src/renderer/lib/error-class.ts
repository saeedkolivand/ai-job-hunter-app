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

/**
 * A length-capped error message, for the failures where the message IS the
 * diagnostic and `errorClass` would destroy it.
 *
 * The export path can drop the message entirely because the Rust side already
 * logs the critical issue codes — the "why" survives elsewhere. A GENERATION
 * failure has no such second source: `Ollama 500: the input length exceeds the
 * context length` is the only record of why a run died, and it is exactly the
 * line that would have resolved the macOS report this PR came from in one look.
 * Reducing it to `Error` would delete the reason this logging exists.
 *
 * So the message is kept, but bounded. A provider/transport error is short; a
 * response that has somehow captured document text is not, and the cap stops
 * that from spilling wholesale into a shareable bundle. `redact_lines`
 * (`commands/support.rs`) then strips URLs, filesystem paths and credential
 * assignments from every line as the bundle is written — it does NOT redact
 * prose, which is what this cap is for. 300 chars matches the cap
 * `friendly_cli_error` already applies to captured CLI stderr, for the same
 * reason.
 */
export function errorDetail(err: unknown, max = 300): string {
  const raw = err instanceof Error ? err.message : String(err);
  return raw.length > max ? `${raw.slice(0, max)}…[truncated]` : raw;
}
