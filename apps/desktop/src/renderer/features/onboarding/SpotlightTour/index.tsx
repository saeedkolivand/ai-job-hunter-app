import {
  Briefcase,
  ClipboardList,
  FilePlus2,
  FileText,
  Gauge,
  LayoutDashboard,
  type LucideIcon,
  Wand2,
  Zap,
} from 'lucide-react';
import { AnimatePresence, motion } from 'motion/react';
import { useCallback, useEffect, useLayoutEffect, useState } from 'react';

import { useTranslation } from '@ajh/translations';
import { Button, StepDots, transition } from '@ajh/ui';

interface TourItem {
  tourId: string;
  icon: LucideIcon;
  titleKey: string;
  descKey: string;
}

// Order follows the sidebar's top-to-bottom flow. Search isn't a tour step —
// it's a ⌘/Ctrl+K shortcut, surfaced in the keyboard-shortcuts cheat sheet.
const TOUR_ITEMS: TourItem[] = [
  {
    tourId: 'dashboard',
    icon: LayoutDashboard,
    titleKey: 'onboarding.tour.dashboard.title',
    descKey: 'onboarding.tour.dashboard.desc',
  },
  {
    tourId: 'applications',
    icon: ClipboardList,
    titleKey: 'onboarding.tour.applications.title',
    descKey: 'onboarding.tour.applications.desc',
  },
  {
    tourId: 'jobs',
    icon: Briefcase,
    titleKey: 'onboarding.tour.jobs.title',
    descKey: 'onboarding.tour.jobs.desc',
  },
  {
    tourId: 'analyze',
    icon: Gauge,
    titleKey: 'onboarding.tour.analyze.title',
    descKey: 'onboarding.tour.analyze.desc',
  },
  {
    tourId: 'generate',
    icon: Wand2,
    titleKey: 'onboarding.tour.generate.title',
    descKey: 'onboarding.tour.generate.desc',
  },
  {
    tourId: 'build',
    icon: FilePlus2,
    titleKey: 'onboarding.tour.build.title',
    descKey: 'onboarding.tour.build.desc',
  },
  {
    tourId: 'documents',
    icon: FileText,
    titleKey: 'onboarding.tour.documents.title',
    descKey: 'onboarding.tour.documents.desc',
  },
  {
    tourId: 'autopilot',
    icon: Zap,
    titleKey: 'onboarding.tour.autopilot.title',
    descKey: 'onboarding.tour.autopilot.desc',
  },
];

interface HighlightRect {
  top: number;
  left: number;
  width: number;
  height: number;
}

interface Props {
  onFinish: () => void;
}

// Cap the rAF measure-retry below so a renamed/removed `data-tour-id` anchor
// fails loud+cheap instead of busy-looping for the tour's lifetime.
const MAX_MEASURE_ATTEMPTS = 60; // ~1s at 60fps — the anchor mounts well before this

export function SpotlightTour({ onFinish }: Props) {
  const { t } = useTranslation();
  const [stepIdx, setStepIdx] = useState(0);
  const [rect, setRect] = useState<HighlightRect | null>(null);

  const current = TOUR_ITEMS[stepIdx] as TourItem;
  const isLast = stepIdx === TOUR_ITEMS.length - 1;

  // The sidebar is forced open right before the tour starts, so its anchors
  // mount and animate width 0 -> auto in the same commit. A single measure
  // can land mid-animation (zero/partial rect that's never re-checked), so
  // retry until the anchor has a real size, then keep tracking it via
  // ResizeObserver until the expand animation settles.
  useLayoutEffect(() => {
    let rafId: number | undefined;
    let observer: ResizeObserver | undefined;
    let attempts = 0;

    const update = (el: Element) => {
      const r = el.getBoundingClientRect();
      setRect({ top: r.top, left: r.left, width: r.width, height: r.height });
    };

    const tryMeasure = () => {
      const el = document.querySelector(`[data-tour-id="${current.tourId}"]`);
      if (el && el.getBoundingClientRect().width > 0) {
        update(el);
        observer = new ResizeObserver(() => update(el));
        observer.observe(el);
      } else if (attempts < MAX_MEASURE_ATTEMPTS) {
        attempts += 1;
        rafId = requestAnimationFrame(tryMeasure);
      } else {
        console.warn(
          `SpotlightTour: anchor [data-tour-id="${current.tourId}"] never measured a real size — stopping retries`
        );
        if (el) update(el); // best-effort — likely zero-size, still better than the last step's rect
      }
    };
    tryMeasure();

    return () => {
      if (rafId !== undefined) cancelAnimationFrame(rafId);
      observer?.disconnect();
    };
  }, [current.tourId]);

  const next = useCallback(() => {
    if (isLast) {
      onFinish();
    } else {
      setStepIdx((i) => i + 1);
    }
  }, [isLast, onFinish]);

  const prev = useCallback(() => {
    setStepIdx((i) => Math.max(0, i - 1));
  }, []);

  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if (e.key === 'Enter' || e.key === 'ArrowRight' || e.key === 'ArrowDown') next();
      if (e.key === 'ArrowLeft' || e.key === 'ArrowUp') prev();
    };
    document.addEventListener('keydown', handler);
    return () => document.removeEventListener('keydown', handler);
  }, [next, prev]);

  const cardTop = rect
    ? Math.max(16, Math.min(rect.top - 16, window.innerHeight - 260))
    : window.innerHeight / 2 - 120;

  const Icon = current.icon;

  return (
    <div className="fixed inset-0 z-[200]">
      {/* Interaction blocker overlay */}
      <div className="absolute inset-0 z-[1]" />

      {/* Spotlight hole — transparent div whose box-shadow creates the dark surround */}
      {rect && (
        <motion.div
          className="absolute z-[2] rounded-xl pointer-events-none"
          animate={{
            top: rect.top - 4,
            left: rect.left - 4,
            width: rect.width + 8,
            height: rect.height + 8,
          }}
          transition={transition.spring}
          style={{
            boxShadow: `0 0 0 9999px var(--color-spotlight-overlay)`,
            border: '1.5px solid rgba(var(--rgb-brand), 0.7)',
          }}
        />
      )}

      {/* Glow pulse behind the spotlight */}
      {rect && (
        <motion.div
          className="absolute z-[2] rounded-xl pointer-events-none"
          animate={{
            top: rect.top - 8,
            left: rect.left - 8,
            width: rect.width + 16,
            height: rect.height + 16,
          }}
          transition={transition.spring}
          style={{
            background:
              'radial-gradient(ellipse at center, rgba(var(--rgb-brand), 0.15) 0%, transparent 70%)',
          }}
        />
      )}

      {/* Info card — appears to the right of the sidebar */}
      <motion.div
        className="absolute z-[3] w-80"
        style={{ left: rect ? rect.left + rect.width + 20 : 276 }}
        animate={{ top: cardTop }}
        transition={transition.spring}
      >
        <div className="glass-modal rounded-2xl p-6">
          {/* Header */}
          <div className="mb-1 flex items-center gap-2 text-[10px] font-semibold uppercase tracking-widest text-foreground/55">
            {t('onboarding.tour.title')}
          </div>

          {/* Feature card */}
          <AnimatePresence mode="wait">
            <motion.div
              key={current.tourId}
              initial={{ opacity: 0, y: 6 }}
              animate={{ opacity: 1, y: 0 }}
              exit={{ opacity: 0, y: -6 }}
              transition={transition.fast}
            >
              {/* Icon + title */}
              <div className="mb-3 flex items-center gap-3">
                <div
                  className="flex h-9 w-9 shrink-0 items-center justify-center rounded-xl"
                  style={{
                    background:
                      'linear-gradient(135deg, rgba(var(--rgb-brand), 0.22) 0%, rgba(var(--rgb-aurora-indigo), 0.12) 100%)',
                    border: '1px solid rgba(var(--rgb-brand), 0.28)',
                  }}
                >
                  <Icon size={16} className="text-brand-soft" />
                </div>
                <h3 className="text-[15px] font-semibold text-foreground/95">
                  {t(current.titleKey)}
                </h3>
              </div>

              {/* Description */}
              <p className="mb-5 text-sm leading-relaxed text-foreground/55">
                {t(current.descKey)}
              </p>
            </motion.div>
          </AnimatePresence>

          {/* Step dots */}
          <div className="mb-4 flex items-center gap-1">
            <StepDots currentStep={stepIdx} totalSteps={TOUR_ITEMS.length} className="my-0" />
            <span className="ml-auto text-[10px] tabular-nums text-foreground/30">
              {t('onboarding.tour.step', {
                current: String(stepIdx + 1),
                total: String(TOUR_ITEMS.length),
              })}
            </span>
          </div>

          {/* Actions */}
          <div className="flex items-center gap-2">
            <Button variant="ghost" onClick={onFinish} className="text-xs">
              {t('onboarding.tour.skip')}
            </Button>
            <Button variant="primary" className="ml-auto" onClick={next}>
              {isLast ? t('onboarding.tour.finish') : t('onboarding.tour.next')}
            </Button>
          </div>
        </div>
      </motion.div>
    </div>
  );
}
