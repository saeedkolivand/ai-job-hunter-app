/**
 * Thin wrapper over the official `ollama` JS client.
 * Centralizes host configuration and adds typed convenience methods.
 */
import { Ollama } from 'ollama';
import type { AiMessage } from '@ajh/shared';
import { execSync } from 'node:child_process';

function getWindowsHostIp(): string | null {
  try {
    const output = execSync('ip route', {
      encoding: 'utf8',
    });

    // Match various ip route output formats
    const match = output.match(/default\s+via\s+([0-9.]+)/);

    if (!match) {
      console.error('[Ollama] Could not parse Windows host IP from ip route output:', output);
    }

    return match?.[1] ?? null;
  } catch (err) {
    console.error('[Ollama] Failed to get Windows host IP:', err);
    return null;
  }
}

function getDefaultHost(): string {
  if (process.env.OLLAMA_HOST) {
    console.warn('[Ollama] Using OLLAMA_HOST from environment:', process.env.OLLAMA_HOST);
    return process.env.OLLAMA_HOST;
  }

  const isWSL = process.platform === 'linux' && !!process.env.WSL_DISTRO_NAME;

  console.warn('[Ollama] Platform:', process.platform, 'WSL detected:', isWSL);

  if (isWSL) {
    const host = getWindowsHostIp();

    if (host) {
      console.warn('[Ollama] Using Windows host IP:', host);
      return `http://${host}:11434`;
    }
    // Fallback for WSL2: try common Windows host IP
    console.warn('[Ollama] Could not detect Windows host IP, trying fallback');
    return 'http://172.0.0.1:11434';
  }

  console.warn('[Ollama] Using localhost');
  return 'http://127.0.0.1:11434';
}

export interface OllamaClientOptions {
  host?: string;
}

export class OllamaClient {
  private readonly client: Ollama;
  constructor(opts: OllamaClientOptions = {}) {
    const host = opts.host ?? process.env.OLLAMA_HOST ?? getDefaultHost();
    this.client = new Ollama({ host });
  }

  async listModels(): Promise<string[]> {
    const res = await this.client.list();
    return res.models.map((m) => m.name);
  }

  async pull(model: string, onProgress?: (p: number) => void): Promise<void> {
    const stream = await this.client.pull({ model, stream: true });
    for await (const chunk of stream) {
      if (chunk.total && chunk.completed) {
        onProgress?.(chunk.completed / chunk.total);
      }
    }
  }

  async chat(
    model: string,
    messages: AiMessage[],
    options: { temperature?: number; signal?: AbortSignal } = {}
  ): Promise<AsyncIterable<{ message: { content: string }; done: boolean }>> {
    return this.client.chat({
      model,
      messages,
      stream: true,
      options: options.temperature !== undefined ? { temperature: options.temperature } : undefined,
    });
  }

  async embed(model: string, input: string | string[]): Promise<number[][]> {
    const res = await this.client.embed({ model, input });
    return res.embeddings;
  }

  async unload(model: string): Promise<void> {
    // Ollama unloads via keep_alive: 0
    await this.client.generate({ model, prompt: '', keep_alive: 0 } as never);
  }
}
