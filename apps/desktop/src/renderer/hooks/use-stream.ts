import { useCallback, useEffect, useRef, useState } from 'react';

import type { AiStreamChunk } from '@ajh/shared';

export interface StreamState {
  text: string;
  isStreaming: boolean;
  clear: () => void;
  append: (delta: string) => void;
}

/**
 * Manages the AI token stream for a single generation session.
 * Subscribes to window.api.ai.onStream and accumulates deltas into `text`.
 * Call `clear()` before starting a new generation to reset the buffer.
 */
export function useStream(): StreamState {
  const [text, setText] = useState('');
  const [isStreaming, setIsStreaming] = useState(false);
  const timeoutRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  const clear = useCallback(() => {
    setText('');
    setIsStreaming(false);
    if (timeoutRef.current) clearTimeout(timeoutRef.current);
  }, []);

  const append = useCallback((delta: string) => {
    setIsStreaming(true);
    setText((prev) => prev + delta);
    if (timeoutRef.current) clearTimeout(timeoutRef.current);
    timeoutRef.current = setTimeout(() => setIsStreaming(false), 500);
  }, []);

  useEffect(() => {
    const offRaw = window.api?.ai.onStream((chunk: unknown) => {
      if ((chunk as AiStreamChunk).done) {
        setIsStreaming(false);
        return;
      }
      if ((chunk as AiStreamChunk).delta) append((chunk as AiStreamChunk).delta);
    });
    const off = offRaw as unknown as (() => void) | undefined;
    return () => {
      off?.();
      if (timeoutRef.current) clearTimeout(timeoutRef.current);
    };
  }, [append]);

  return { text, isStreaming, clear, append };
}
