import { useEffect } from 'react';
import { createPortal } from 'react-dom';
import { AnimatePresence, motion } from 'motion/react';
import { type LucideIcon, CheckCircle2, XCircle, Info, AlertTriangle, X } from 'lucide-react';
import { transition } from '../lib/motion';

export type ToastVariant = 'success' | 'error' | 'info' | 'warning';

interface ToastProps {
  open: boolean;
  onClose: () => void;
  message: string;
  variant?: ToastVariant;
  duration?: number;
}

const VARIANTS: Record<
  ToastVariant,
  { icon: LucideIcon; iconBg: string; glow: string; ambientColor: string }
> = {
  success: {
    icon: CheckCircle2,
    iconBg: 'bg-emerald-500',
    glow: '0 0 60px rgba(16,185,129,0.25)',
    ambientColor: 'rgba(16,185,129,0.12)',
  },
  error: {
    icon: XCircle,
    iconBg: 'bg-red-500',
    glow: '0 0 60px rgba(239,68,68,0.25)',
    ambientColor: 'rgba(239,68,68,0.12)',
  },
  info: {
    icon: Info,
    iconBg: 'bg-blue-500',
    glow: '0 0 60px rgba(59,130,246,0.25)',
    ambientColor: 'rgba(59,130,246,0.12)',
  },
  warning: {
    icon: AlertTriangle,
    iconBg: 'bg-amber-500',
    glow: '0 0 60px rgba(245,158,11,0.25)',
    ambientColor: 'rgba(245,158,11,0.12)',
  },
};

export function Toast({ open, onClose, message, variant = 'info', duration = 5000 }: ToastProps) {
  const cfg = VARIANTS[variant];
  const Icon = cfg.icon;

  useEffect(() => {
    if (!open || duration <= 0) return;
    const t = setTimeout(onClose, duration);
    return () => clearTimeout(t);
  }, [open, duration, onClose]);

  return createPortal(
    <AnimatePresence>
      {open && (
        <motion.div
          key="toast"
          className="fixed bottom-6 right-6 z-[10000]"
          initial={{ opacity: 0, y: 16, scale: 0.96 }}
          animate={{ opacity: 1, y: 0, scale: 1 }}
          exit={{ opacity: 0, y: 12, scale: 0.97 }}
          transition={transition.relaxed}
        >
          {/* Ambient glow */}
          <div
            className="pointer-events-none absolute -left-6 -top-6 h-24 w-24 rounded-full blur-2xl"
            style={{ background: cfg.ambientColor }}
          />

          {/* Panel */}
          <div
            className="toast-panel relative flex w-[420px] items-center gap-4 rounded-2xl px-4 py-4"
            style={{
              boxShadow: `0 16px 48px rgba(0,0,0,0.45), ${cfg.glow}, inset 0 1px 0 rgba(255,255,255,0.06)`,
            }}
          >
            {/* Top hairline */}
            <div className="absolute inset-x-0 top-0 h-px rounded-t-2xl bg-gradient-to-r from-transparent via-white/10 to-transparent" />

            {/* Icon */}
            <div
              className={`flex h-12 w-12 shrink-0 items-center justify-center rounded-xl shadow-lg ${cfg.iconBg} text-white`}
            >
              <Icon size={24} strokeWidth={2} />
            </div>

            {/* Message */}
            <p className="flex-1 text-[15px] font-medium leading-snug text-white/90">{message}</p>

            {/* Close */}
            <button
              onClick={onClose}
              className="flex h-9 w-9 shrink-0 items-center justify-center rounded-full bg-white/10 text-white/50 transition-all hover:bg-white/15 hover:text-white/80"
            >
              <X size={15} strokeWidth={2.5} />
            </button>
          </div>
        </motion.div>
      )}
    </AnimatePresence>,
    document.body
  );
}
