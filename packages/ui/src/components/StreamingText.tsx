import { useEffect, useRef } from 'react';
import { cn } from '../lib/cn';

interface StreamingTextProps {
  text: string;
  isStreaming?: boolean;
  autoScroll?: boolean;
  className?: string;
  tail?: number;
}

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
