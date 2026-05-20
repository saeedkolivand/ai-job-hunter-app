import { useEffect, useRef } from 'react';
import { cn } from '@/lib/cn';

interface StreamingTextProps {
  text: string;
  isStreaming?: boolean;
  /** Auto-scroll the container to the bottom as text grows. Default true. */
  autoScroll?: boolean;
  className?: string;
  /** Limit displayed characters from the end (useful for large outputs). */
  tail?: number;
}

/**
 * Renders live-streaming AI text with an animated cursor.
 *
 * - Respects prefers-reduced-motion: cursor doesn't blink.
 * - Auto-scrolls to the bottom as text grows (can be disabled).
 * - Optionally trims to a tail window to keep DOM small for long outputs.
 */
export function StreamingText({
  text,
  isStreaming = false,
  autoScroll = true,
  className,
  tail,
}: StreamingTextProps) {
  const bottomRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (autoScroll && isStreaming) {
      bottomRef.current?.scrollIntoView({ behavior: 'smooth', block: 'end' });
    }
  }, [text, isStreaming, autoScroll]);

  const displayed = tail && text.length > tail ? text.slice(-tail) : text;

  return (
    <div className={cn('relative', className)}>
      <span className="whitespace-pre-wrap break-words text-sm leading-relaxed text-foreground/85">
        {displayed}
        {isStreaming && (
          <span
            className={cn(
              'ml-0.5 inline-block h-4 w-0.5 translate-y-0.5 rounded-sm bg-brand-soft',
              'motion-safe:animate-[blink_0.9s_ease-in-out_infinite]'
            )}
            aria-hidden="true"
          />
        )}
      </span>
      <div ref={bottomRef} />
    </div>
  );
}
