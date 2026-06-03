import { Clock } from 'lucide-react';
import { AnimatePresence, motion } from 'motion/react';
import { useEffect, useRef, useState } from 'react';

import { ProgressBar, transition } from '@ajh/ui';

import { ThinkingBubble } from '@/features/ai-generate/components/ThinkingBubble';

interface AnalysisProgressProps {
  running: boolean;
  stream: string;
  thinking?: string;
  modelLoading?: boolean;
  tokenCount?: number;
  tokenStartMs?: number | null;
  /** Expected duration (ms) for the active provider — drives the bar + ETA. */
  estimatedMs?: number;
  /** Slow provider (CLI agent / local) — show an up-front "this can take a while" hint. */
  slow?: boolean;
  t: (key: string) => string;
}

export function AnalysisProgress({
  running,
  stream,
  thinking,
  modelLoading,
  tokenCount,
  tokenStartMs,
  estimatedMs,
  slow,
  t,
}: AnalysisProgressProps) {
  const PROGRESS_MESSAGES = [
    t('analyze.progress.reading'),
    t('analyze.progress.scanning'),
    t('analyze.progress.calculating'),
    t('analyze.progress.checking'),
    t('analyze.progress.measuring'),
    t('analyze.progress.identifying'),
    t('analyze.progress.prioritising'),
    t('analyze.progress.scoring'),
    t('analyze.progress.generating'),
    t('analyze.progress.risks'),
    t('analyze.progress.writing'),
    t('analyze.progress.finalising'),
  ];

  // Expected duration for the active provider; the bar eases to 90% by here, then
  // crawls. Provider-aware so a slow CLI/local run doesn't read "almost done" for
  // minutes.
  const estMs = estimatedMs ?? 50_000;

  const [progress, setProgress] = useState(0); // 0–100
  const [elapsed, setElapsed] = useState(0); // seconds
  const [msgIdx, setMsgIdx] = useState(0);
  const startRef = useRef(Date.now());

  useEffect(() => {
    if (!running) return;

    startRef.current = Date.now();
    setProgress(0);
    setElapsed(0);
    setMsgIdx(0);

    const tick = setInterval(() => {
      const ms = Date.now() - startRef.current;
      setElapsed(Math.floor(ms / 1000));
      // Ease toward 90% over estMs, then crawl slowly after (capped below 100%).
      const ratio = Math.min(ms / estMs, 1);
      const eased = ratio < 1 ? 90 * (1 - Math.pow(1 - ratio, 3)) : 90 + (ms - estMs) / 4000;
      setProgress(Math.min(eased, 97));
    }, 400);

    const msgTick = setInterval(() => {
      setMsgIdx((i) => (i + 1) % PROGRESS_MESSAGES.length);
    }, 3500);

    return () => {
      clearInterval(tick);
      clearInterval(msgTick);
    };
  }, [running, estMs, PROGRESS_MESSAGES.length]);

  const mins = Math.floor(elapsed / 60);
  const secs = elapsed % 60;
  const timer = mins > 0 ? `${mins}m ${secs}s` : `${secs}s`;

  const estSec = Math.round(estMs / 1000);
  const fmtLeft = (s: number) =>
    s >= 60 ? `~${Math.floor(s / 60)}m ${s % 60}s left` : `~${s}s left`;
  // Honest ETA: count down toward the estimate, then admit it's running long
  // rather than showing a stuck "almost done".
  const eta =
    elapsed >= estSec
      ? t('analyze.progress.takingLonger')
      : elapsed > 5
        ? fmtLeft(Math.max(1, estSec - elapsed))
        : '';

  return (
    <div className="mt-4 rounded-xl border border-white/[0.07] bg-white/[0.02] px-6 py-6 space-y-5">
      {/* Top row */}
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-2">
          <span className="h-1.5 w-1.5 animate-pulse rounded-full bg-brand" />
          <span className="text-[10px] font-semibold uppercase tracking-[0.18em] text-foreground/55">
            {t('analyze.running')}
          </span>
        </div>
        <div className="flex items-center gap-3 text-[10px] text-foreground/30">
          {eta && <span className="text-brand-soft/70">{eta}</span>}
          <span>{timer}</span>
        </div>
      </div>

      {/* Up-front expectation for slow providers (CLI agents / local models) */}
      {slow && (
        <div className="flex items-center gap-1.5 text-[10px] text-foreground/35">
          <Clock size={11} className="text-brand-soft/50" />
          {t('analyze.progress.slowHint')}
        </div>
      )}

      {/* Rotating message */}
      <div className="relative h-6 overflow-hidden">
        <AnimatePresence mode="wait">
          <motion.p
            key={msgIdx}
            initial={{ opacity: 0, y: 10 }}
            animate={{ opacity: 1, y: 0 }}
            exit={{ opacity: 0, y: -10 }}
            transition={transition.slow}
            className="absolute inset-0 flex items-center text-sm text-foreground/60"
          >
            {PROGRESS_MESSAGES[msgIdx]}
          </motion.p>
        </AnimatePresence>
      </div>

      {/* Progress bar */}
      <ProgressBar value={progress} height={6} showLabel />

      {/* Thinking output */}
      {thinking && <ThinkingBubble thinking={thinking} done={stream.length > 0} />}

      {/* Model loading indicator */}
      {modelLoading && (
        <div className="flex items-center gap-2 text-[10px] text-foreground/40">
          <span className="h-1.5 w-1.5 animate-spin rounded-full border border-brand border-t-transparent" />
          Loading model into memory...
        </div>
      )}

      {/* Live stream output */}
      <div className="rounded-lg border border-white/[0.05] bg-black/20 px-4 py-3 h-28 overflow-hidden relative">
        {stream ? (
          <pre className="font-mono text-[10px] leading-relaxed text-foreground/30 whitespace-pre-wrap break-all">
            {stream.slice(-800)}
          </pre>
        ) : (
          <div className="space-y-2 pt-1">
            {[1, 0.7, 0.85, 0.5].map((w, i) => (
              <div
                key={i}
                className="h-2 rounded-full bg-white/[0.05] animate-pulse"
                style={{ width: `${w * 100}%`, animationDelay: `${i * 150}ms` }}
              />
            ))}
          </div>
        )}
        {/* Fade out top so it looks like it's scrolling in from below */}
        <div className="absolute inset-x-0 top-0 h-8 bg-gradient-to-b from-black/20 to-transparent pointer-events-none" />
      </div>

      {/* Token throughput */}
      {tokenCount != null &&
        tokenCount > 0 &&
        (() => {
          const tokElapsed = tokenStartMs ? (Date.now() - tokenStartMs) / 1000 : 0;
          const tokPerSec = tokElapsed > 2 ? Math.round(tokenCount / tokElapsed) : null;
          return (
            <div className="text-[10px] text-foreground/25 text-right">
              {tokenCount.toLocaleString()} tokens{tokPerSec ? ` · ~${tokPerSec} tok/s` : ''}
            </div>
          );
        })()}
    </div>
  );
}
