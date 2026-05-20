/**
 * Streaming chat helper — locale-aware. The system prompt enforces that the
 * assistant ALWAYS responds in the selected application locale, regardless
 * of the source document language.
 */
import type { AiGenerateRequest } from '@ajh/shared';
import type { OllamaClient } from '../client/ollama.js';

const LOCALE_NAMES: Record<string, string> = {
  en: 'English',
  de: 'German',
  fr: 'French',
  es: 'Spanish',
  it: 'Italian',
  tr: 'Turkish',
  pt: 'Portuguese',
  ru: 'Russian',
  zh: 'Chinese',
  ja: 'Japanese',
  ko: 'Korean',
};

export async function* generateStream(
  client: OllamaClient,
  req: AiGenerateRequest,
  signal?: AbortSignal
): AsyncGenerator<{ delta: string; done: boolean }> {
  const localeName = LOCALE_NAMES[req.locale] ?? 'English';
  const systemPreamble = `You are an AI assistant inside a local desktop application. ALWAYS respond in ${localeName}, regardless of the language of any source documents or user-provided content. Be concise, factual, and well-formatted.`;

  const messages = [
    { role: 'system' as const, content: systemPreamble },
    ...req.messages.filter((m) => m.role !== 'system'),
  ];

  const stream = await client.chat(req.model, messages, {
    ...(req.temperature !== undefined ? { temperature: req.temperature } : {}),
    ...(signal ? { signal } : {}),
  });

  for await (const chunk of stream) {
    if (signal?.aborted) return;
    yield { delta: chunk.message.content ?? '', done: chunk.done };
    if (chunk.done) return;
  }
}
