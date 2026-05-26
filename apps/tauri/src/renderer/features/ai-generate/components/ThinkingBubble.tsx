import { Brain, ChevronDown } from 'lucide-react';
import { AnimatePresence, motion } from 'motion/react';
import { useEffect, useRef, useState } from 'react';

import { transition } from '@/lib/motion';

interface ThinkingBubbleProps {
  thinking: string;
  done?: boolean;
}

export function ThinkingBubble({ thinking, done = false }: ThinkingBubbleProps) {
  const [expanded, setExpanded] = useState(true);
  const scrollRef = useRef<HTMLPreElement>(null);

  useEffect(() => {
    if (done && thinking) setExpanded(false);
  }, [done, thinking]);

  useEffect(() => {
    if (expanded && scrollRef.current) {
      scrollRef.current.scrollTop = scrollRef.current.scrollHeight;
    }
  }, [thinking, expanded]);

  if (!thinking) return null;

  const wordCount = thinking.trim().split(/\s+/).length;
  const excerpt = thinking.replace(/\n/g, ' ').trim().slice(0, 80);
  const showExcerpt = !expanded && excerpt.length > 0;

  return (
    <motion.div
      initial={{ opacity: 0, y: -6 }}
      animate={{ opacity: 1, y: 0 }}
      transition={transition.normal}
      className="mx-1 mb-3 rounded-xl border border-violet-500/20 bg-violet-500/[0.04]"
    >
      <button
        onClick={() => setExpanded((v) => !v)}
        className="flex w-full items-center gap-2 px-3 py-2 text-left"
      >
        <Brain size={12} className="shrink-0 text-violet-400/70" />
        <span className="flex-1 min-w-0 flex items-baseline gap-2">
          <span className="text-[10px] font-semibold uppercase tracking-[0.16em] text-violet-400/60 shrink-0">
            {done ? 'Reasoning complete' : 'Thinking…'}
          </span>
          {showExcerpt && (
            <span className="truncate text-[10px] text-violet-300/30">
              {excerpt}
              {excerpt.length < thinking.trim().replace(/\n/g, ' ').length ? '…' : ''}
            </span>
          )}
        </span>
        <span className="shrink-0 text-[9px] tabular-nums text-violet-400/30">{wordCount}w</span>
        {!done && (
          <span className="flex gap-0.5 shrink-0">
            {[0, 150, 300].map((delay) => (
              <span
                key={delay}
                className="h-1 w-1 animate-pulse rounded-full bg-violet-400/50"
                style={{ animationDelay: `${delay}ms` }}
              />
            ))}
          </span>
        )}
        <motion.div animate={{ rotate: expanded ? 0 : -90 }} transition={transition.fast}>
          <ChevronDown size={12} className="shrink-0 text-violet-400/40" />
        </motion.div>
      </button>

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
              className="max-h-40 overflow-y-auto whitespace-pre-wrap break-all px-3 pb-3 font-mono text-[10px] leading-relaxed text-violet-300/40"
            >
              {thinking}
              {!done && (
                <span className="ml-0.5 inline-block h-2.5 w-0.5 animate-pulse bg-violet-400/50 align-middle" />
              )}
            </pre>
          </motion.div>
        )}
      </AnimatePresence>
    </motion.div>
  );
}
