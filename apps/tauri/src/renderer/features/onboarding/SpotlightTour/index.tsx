import {
  Briefcase,
  FileText,
  Gauge,
  LayoutDashboard,
  type LucideIcon,
  Search,
  Sparkles,
  Wand2,
  Zap,
} from 'lucide-react';
import { AnimatePresence, motion } from 'motion/react';
import { useCallback, useEffect, useLayoutEffect, useState } from 'react';

import { Button, cn, transition } from '@ajh/ui';

import { useTranslation } from '@/lib/i18n';

interface TourItem {
  tourId: string;
  icon: LucideIcon;
  titleKey: string;
  descKey: string;
}

const TOUR_ITEMS: TourItem[] = [
  {
    tourId: 'dashboard',
    icon: LayoutDashboard,
    titleKey: 'onboarding.tour.dashboard.title',
    descKey: 'onboarding.tour.dashboard.desc',
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
    tourId: 'jobs',
    icon: Briefcase,
    titleKey: 'onboarding.tour.jobs.title',
    descKey: 'onboarding.tour.jobs.desc',
  },
  {
    tourId: 'autopilot',
    icon: Zap,
    titleKey: 'onboarding.tour.autopilot.title',
    descKey: 'onboarding.tour.autopilot.desc',
  },
  {
    tourId: 'documents',
    icon: FileText,
    titleKey: 'onboarding.tour.documents.title',
    descKey: 'onboarding.tour.documents.desc',
  },
  {
    tourId: 'search',
    icon: Search,
    titleKey: 'onboarding.tour.search.title',
    descKey: 'onboarding.tour.search.desc',
  },
  {
    tourId: 'ai',
    icon: Sparkles,
    titleKey: 'onboarding.tour.ai.title',
    descKey: 'onboarding.tour.ai.desc',
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

export function SpotlightTour({ onFinish }: Props) {
  const { t } = useTranslation();
  const [stepIdx, setStepIdx] = useState(0);
  const [rect, setRect] = useState<HighlightRect | null>(null);

  const current = TOUR_ITEMS[stepIdx] as TourItem;
  const isLast = stepIdx === TOUR_ITEMS.length - 1;

  const measureTarget = useCallback((id: string) => {
    const el = document.querySelector(`[data-tour-id="${id}"]`);
    if (el) {
      const r = el.getBoundingClientRect();
      setRect({ top: r.top, left: r.left, width: r.width, height: r.height });
    }
  }, []);

  useLayoutEffect(() => {
    measureTarget(current.tourId);
  }, [current.tourId, measureTarget]);

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
            boxShadow: '0 0 0 9999px rgba(0,0,0,0.76)',
            border: '1.5px solid rgba(168,85,247,0.7)',
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
              'radial-gradient(ellipse at center, rgba(168,85,247,0.15) 0%, transparent 70%)',
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
        <div
          className="rounded-2xl border border-white/[0.08] p-6"
          style={{
            background: 'linear-gradient(145deg, rgba(20,14,36,0.98) 0%, rgba(12,10,24,0.98) 100%)',
            boxShadow:
              '0 24px 60px rgba(0,0,0,0.55), 0 0 0 1px rgba(168,85,247,0.1), inset 0 1px 0 rgba(255,255,255,0.04)',
          }}
        >
          {/* Header */}
          <div className="mb-1 flex items-center gap-1 text-[10px] font-semibold uppercase tracking-widest text-foreground/30">
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
                      'linear-gradient(135deg, rgba(168,85,247,0.22) 0%, rgba(99,102,241,0.12) 100%)',
                    border: '1px solid rgba(168,85,247,0.28)',
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
            {TOUR_ITEMS.map((_, i) => (
              <Button
                key={i}
                onClick={() => setStepIdx(i)}
                className={cn(
                  'h-1 rounded-full transition-all duration-300 p-0 border-transparent',
                  i === stepIdx ? 'w-5 bg-brand' : 'w-1.5 bg-white/15 hover:bg-white/25'
                )}
              />
            ))}
            <span className="ml-auto text-[10px] tabular-nums text-foreground/30">
              {t('onboarding.tour.step', {
                current: String(stepIdx + 1),
                total: String(TOUR_ITEMS.length),
              })}
            </span>
          </div>

          {/* Actions */}
          <div className="flex items-center gap-2">
            <Button
              onClick={onFinish}
              className="text-xs text-foreground/30 transition-colors hover:text-foreground/55 h-auto bg-transparent border-transparent"
            >
              {t('onboarding.tour.skip')}
            </Button>
            <Button variant="default" size="sm" className="ml-auto" onClick={next}>
              {isLast ? t('onboarding.tour.finish') : t('onboarding.tour.next')}
            </Button>
          </div>
        </div>
      </motion.div>
    </div>
  );
}
