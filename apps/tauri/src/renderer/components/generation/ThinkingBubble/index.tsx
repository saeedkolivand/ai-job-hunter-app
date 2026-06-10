import { Brain, ChevronDown } from 'lucide-react';
import { AnimatePresence, motion } from 'motion/react';
import { useEffect, useRef, useState } from 'react';

import { Button, transition } from '@ajh/ui';

interface ThinkingBubbleProps {
  thinking: string;
  done?: boolean;
}

export function ThinkingBubble({ thinking, done = false }: ThinkingBubbleProps) {
  const [expanded, setExpanded] = useState(true);
  const scrollRef = useRef<HTMLPreElement>(null);

  // Auto-collapse when the model finishes thinking and real output starts
  useEffect(() => {
    if (done && thinking) setExpanded(false);
  }, [done, thinking]);

  // Keep scroll pinned to bottom while streaming
  useEffect(() => {
    if (expanded && scrollRef.current) {
      scrollRef.current.scrollTop = scrollRef.current.scrollHeight;
    }
  }, [thinking, expanded]);

  if (!thinking) return null;

  return (
    <motion.div
      initial={{ opacity: 0, y: -6 }}
      animate={{ opacity: 1, y: 0 }}
      transition={transition.normal}
      className="mx-1 mb-3 rounded-xl border border-violet-500/20 bg-violet-500/[0.04]"
    >
      {/* Header */}
      <Button
        variant="unstyled"
        aria-expanded={expanded}
        onClick={() => setExpanded((v) => !v)}
        className="flex w-full items-center gap-2 px-3 py-2 text-left"
      >
        <Brain size={12} className="shrink-0 text-violet-400/70" />
        <span className="flex-1 text-[10px] font-semibold uppercase tracking-[0.16em] text-violet-400/60">
          {done ? 'Reasoning complete' : 'Thinking…'}
        </span>
        {!done && (
          <span className="flex gap-0.5">
            {[0, 150, 300].map((delay) => (
              <span
                key={delay}
                className="h-1 w-1 rounded-full bg-violet-400/50 animate-pulse"
                style={{ animationDelay: `${delay}ms` }}
              />
            ))}
          </span>
        )}
        <motion.div animate={{ rotate: expanded ? 0 : -90 }} transition={transition.fast}>
          <ChevronDown size={12} className="text-violet-400/40" />
        </motion.div>
      </Button>

      {/* Content */}
      <AnimatePresence initial={false}>
        {expanded && (
          <motion.div
            key="content"
            initial={{ height: 0, opacity: 0 }}
            animate={{ height: 'auto', opacity: 1 }}
            exit={{ height: 0, opacity: 0 }}
            transition={transition.normal}
            className="overflow-hidden"
          >
            <pre
              ref={scrollRef}
              className="select-text max-h-40 overflow-y-auto px-3 pb-3 font-mono text-[10px] leading-relaxed text-violet-300/40 whitespace-pre-wrap break-all"
            >
              {thinking}
              {!done && (
                <span className="inline-block h-2.5 w-0.5 animate-pulse bg-violet-400/50 ml-0.5 align-middle" />
              )}
            </pre>
          </motion.div>
        )}
      </AnimatePresence>
    </motion.div>
  );
}
