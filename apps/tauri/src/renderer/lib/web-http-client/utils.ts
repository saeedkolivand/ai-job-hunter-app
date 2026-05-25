export interface WebHttpClientOptions {
  /** Base URL of the runtime server (e.g. http://127.0.0.1:8742). */
  baseUrl: string;
  /**
   * Optional per-launch auth token placed in the Authorization header.
   * The runtime server generates this on startup and the shell passes it
   * to the renderer via a secure channel (IPC, environment variable, etc.).
   */
  token?: string;
}

type UnsubFn = () => void;

export function createHttpClientHelpers({ baseUrl, token }: WebHttpClientOptions) {
  const base = baseUrl.replace(/\/$/, '');
  const headers: Record<string, string> = { 'Content-Type': 'application/json' };
  if (token) headers['Authorization'] = `Bearer ${token}`;

  /** POST /api/<namespace>/<method> and return parsed JSON. */
  async function cmd<T = never>(namespace: string, method: string, payload?: unknown): Promise<T> {
    const res = await fetch(`${base}/api/${namespace}/${method}`, {
      method: 'POST',
      headers,
      body: payload !== undefined ? JSON.stringify(payload) : undefined,
    });
    if (!res.ok) {
      const msg = await res.text().catch(() => res.statusText);
      throw new Error(`[web-http] ${namespace}.${method} → ${res.status}: ${msg}`);
    }
    return res.json() as Promise<T>;
  }

  /**
   * Subscribe to a Server-Sent Events channel.
   * Returns a sync unsubscribe function (closes the EventSource).
   */
  function subscribe<T>(channel: string, handler: (event: T) => void): UnsubFn {
    const url = new URL(`${base}/api/events/${channel}`);
    if (token) url.searchParams.set('token', token);

    const es = new EventSource(url.toString());
    es.onmessage = (e) => {
      try {
        handler(JSON.parse(e.data as string) as T);
      } catch {
        // malformed event — ignore
      }
    };
    es.onerror = () => es.close();
    return () => es.close();
  }

  return { cmd, subscribe };
}
