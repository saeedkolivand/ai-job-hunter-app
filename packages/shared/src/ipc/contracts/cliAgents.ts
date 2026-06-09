/** Per-agent install status for the Settings → AI "CLI agents" panel (#22). */
export interface CliAgentStatus {
  /** Provider id (`claude-code` | `codex` | `gemini-cli`). */
  id: string;
  /** Binary looked up on PATH (e.g. `claude`). */
  binary: string;
  installed: boolean;
  version: string | null;
  /** npm package that provides the binary (shown in the guide). */
  package: string;
  /** Official install/setup docs, opened by the guide path. */
  docsUrl: string;
  /** Shell-capability command name for the one-click install (`install-<id>`). */
  installCommandName: string;
  /** Exact args to pass `install` — must match the capability allowlist entry. */
  installArgs: string[];
}

export interface CliAgentsStatus {
  agents: CliAgentStatus[];
  /** `npm` on PATH — gates the one-click install (the guide always shows). */
  npmAvailable: boolean;
}

/** Result of a one-click install spawn. */
export interface CliAgentInstallResult {
  /** Process exit code (`null` if killed). `0` = success. */
  code: number | null;
  success: boolean;
}

export interface CliAgentsContract {
  /** Cached install status for every CLI agent (+ npm availability). */
  status(): Promise<CliAgentsStatus>;

  /** Clear the detection cache and re-probe (call after an install). */
  redetect(): Promise<CliAgentsStatus>;

  /**
   * One-click install: spawn the capability-allowlisted command (fixed args) and
   * stream its output. Implemented over the shell plugin in the adapter — the
   * caller can't tell it isn't a plain IPC command. `commandName`/`args` come
   * verbatim from {@link CliAgentStatus}; the shell capability rejects anything
   * not in the static allowlist.
   */
  install(opts: {
    commandName: string;
    args: string[];
    onOutput?: (line: string) => void;
    signal?: AbortSignal;
  }): Promise<CliAgentInstallResult>;
}

export const CLI_AGENTS_CHANNELS = {
  status: 'cliAgents:status',
  redetect: 'cliAgents:redetect',
} as const;
