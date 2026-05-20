import { createContext, useContext, useCallback, useState, useEffect, useRef } from 'react';
import { createPortal } from 'react-dom';
import { AnimatePresence, motion } from 'motion/react';
import { type LucideIcon, CheckCircle2, XCircle, Info, AlertTriangle, X } from 'lucide-react';
import { transition } from '../lib/motion';

export type ToastVariant = 'success' | 'error' | 'info' | 'warning';

export interface ToastItem {
  id: string;
  message: string;
  variant: ToastVariant;
  duration: number;
}

interface ToastContextValue {
  toast: (message: string, variant?: ToastVariant, duration?: number) => void;
}

const ToastContext = createContext<ToastContextValue | null>(null);

const VARIANT_CFG: Record<
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

function ToastCard({ item, onClose }: { item: ToastItem; onClose: () => void }) {
  const cfg = VARIANT_CFG[item.variant];
  const Icon = cfg.icon;
  const onCloseRef = useRef(onClose);
  onCloseRef.current = onClose;

  useEffect(() => {
    if (item.duration <= 0) return;
    const t = setTimeout(() => onCloseRef.current(), item.duration);
    return () => clearTimeout(t);
  }, [item.id, item.duration]);

  return (
    <motion.div
      layout
      initial={{ opacity: 0, y: 20, scale: 0.96 }}
      animate={{ opacity: 1, y: 0, scale: 1 }}
      exit={{ opacity: 0, y: 12, scale: 0.97 }}
      transition={transition.relaxed}
    >
      <div
        className="pointer-events-none absolute -left-6 -top-6 h-24 w-24 rounded-full blur-2xl"
        style={{ background: cfg.ambientColor }}
      />
      <div
        className="toast-panel relative flex w-[420px] items-center gap-4 rounded-2xl px-4 py-4"
        style={{
          boxShadow: `0 16px 48px rgba(0,0,0,0.45), ${cfg.glow}, inset 0 1px 0 rgba(255,255,255,0.06)`,
        }}
      >
        <div className="absolute inset-x-0 top-0 h-px rounded-t-2xl bg-gradient-to-r from-transparent via-white/10 to-transparent" />
        <div
          className={`flex h-12 w-12 shrink-0 items-center justify-center rounded-xl shadow-lg ${cfg.iconBg} text-white`}
        >
          <Icon size={24} strokeWidth={2} />
        </div>
        <p className="flex-1 text-[15px] font-medium leading-snug text-white/90">{item.message}</p>
        <button
          onClick={onClose}
          className="flex h-9 w-9 shrink-0 items-center justify-center rounded-full bg-white/10 text-white/50 transition-all hover:bg-white/15 hover:text-white/80"
        >
          <X size={15} strokeWidth={2.5} />
        </button>
      </div>
    </motion.div>
  );
}

function Toaster({ toasts, dismiss }: { toasts: ToastItem[]; dismiss: (id: string) => void }) {
  return createPortal(
    <div className="fixed bottom-6 right-6 z-[var(--z-toast)] flex flex-col-reverse gap-3">
      <AnimatePresence initial={false}>
        {toasts.map((item) => (
          <ToastCard key={item.id} item={item} onClose={() => dismiss(item.id)} />
        ))}
      </AnimatePresence>
    </div>,
    document.body
  );
}

export function ToastProvider({ children }: { children: React.ReactNode }) {
  const [toasts, setToasts] = useState<ToastItem[]>([]);

  const dismiss = useCallback((id: string) => {
    setToasts((prev) => prev.filter((t) => t.id !== id));
  }, []);

  const toast = useCallback((message: string, variant: ToastVariant = 'info', duration = 5000) => {
    const id = `${Date.now()}-${Math.random()}`;
    setToasts((prev) => [...prev, { id, message, variant, duration }]);
  }, []);

  return (
    <ToastContext.Provider value={{ toast }}>
      {children}
      <Toaster toasts={toasts} dismiss={dismiss} />
    </ToastContext.Provider>
  );
}

export function useToast() {
  const ctx = useContext(ToastContext);
  if (!ctx) throw new Error('useToast must be used within ToastProvider');
  return ctx.toast;
}
