import { useEffect, useRef, useState } from 'react';

/**
 * Clipboard copy with transient "copied" feedback: the flag flips on for `ms`
 * then resets. The timer is cleared on unmount so it can't setState afterwards.
 */
export function useCopyFlag(ms: number): [boolean, (text: string) => void] {
  const [copied, setCopied] = useState(false);
  const timerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  useEffect(() => () => clearTimeout(timerRef.current ?? undefined), []);

  const copy = (text: string) => {
    void navigator.clipboard
      .writeText(text)
      .then(() => {
        setCopied(true);
        if (timerRef.current) clearTimeout(timerRef.current);
        timerRef.current = setTimeout(() => setCopied(false), ms);
      })
      .catch(() => {});
  };
  return [copied, copy];
}
