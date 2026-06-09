import { invoke } from '@tauri-apps/api/core';
import { Command } from '@tauri-apps/plugin-shell';

import type { CliAgentInstallResult } from '@ajh/shared/ipc';

/**
 * CLI-agent install adapter (#22). `status`/`redetect` are plain Rust commands;
 * `install` is the one place the shell plugin is used — it spawns the
 * capability-allowlisted command (fixed args from `status`, validated by the shell
 * scope) and streams its output. The renderer consumes all of this via the
 * `cliAgents` service hook and can't tell install isn't a plain IPC call.
 */
export const cliAgents = {
  status: () => invoke('cli_agents_status'),
  redetect: () => invoke('cli_agents_redetect'),

  install: ({
    commandName,
    args,
    onOutput,
    signal,
  }: {
    commandName: string;
    args: string[];
    onOutput?: (line: string) => void;
    signal?: AbortSignal;
  }): Promise<CliAgentInstallResult> =>
    new Promise<CliAgentInstallResult>((resolve, reject) => {
      const command = Command.create(commandName, args);
      command.stdout.on('data', (line) => onOutput?.(String(line)));
      command.stderr.on('data', (line) => onOutput?.(String(line)));
      command.on('error', (err) => reject(new Error(String(err))));
      command.on('close', (data) => {
        const code = data.code ?? null;
        resolve({ code, success: code === 0 });
      });
      command
        .spawn()
        .then((child) => {
          signal?.addEventListener('abort', () => void child.kill().catch(() => {}));
        })
        .catch(reject);
    }),
};
