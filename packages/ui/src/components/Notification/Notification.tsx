import { AlertTriangle, CheckCircle2, Info, type LucideIcon, X, XCircle } from 'lucide-react';
import { AnimatePresence, motion } from 'motion/react';
import { createContext, useCallback, useContext, useEffect, useRef, useState } from 'react';
import { createPortal } from 'react-dom';

import { transition } from '../../lib/motion';

// ─── Types ────────────────────────────────────────────────────────────────────

export type NotificationVariant = 'success' | 'error' | 'info' | 'warning';

export interface NotificationItem {
  id: string;
  message: string;
  variant: NotificationVariant;
  duration: number;
}

interface NotificationContextValue {
  notify: (message: string, variant?: NotificationVariant, duration?: number) => void;
}

const NotificationContext = createContext<NotificationContextValue | null>(null);

// ─── Variant config ───────────────────────────────────────────────────────────

const VARIANTS: Record<
  NotificationVariant,
  { icon: LucideIcon; iconBg: string; glow: string; ambient: string }
> = {
  success: {
    icon: CheckCircle2,
    iconBg: '#10b981',
    glow: 'rgba(16,185,129,0.30)',
    ambient: 'rgba(16,185,129,0.14)',
  },
  error: {
    icon: XCircle,
    iconBg: '#ef4444',
    glow: 'rgba(239,68,68,0.30)',
    ambient: 'rgba(239,68,68,0.14)',
  },
  info: {
    icon: Info,
    iconBg: '#3b82f6',
    glow: 'rgba(59,130,246,0.30)',
    ambient: 'rgba(59,130,246,0.14)',
  },
  warning: {
    icon: AlertTriangle,
    iconBg: '#f59e0b',
    glow: 'rgba(245,158,11,0.30)',
    ambient: 'rgba(245,158,11,0.14)',
  },
};

// ─── Card ─────────────────────────────────────────────────────────────────────

function NotificationCard({ item, onClose }: { item: NotificationItem; onClose: () => void }) {
  const cfg = VARIANTS[item.variant];
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
      style={{ position: 'relative' }}
    >
      {/* Panel — inline styles so backdrop-filter always works in portals */}
      <div
        style={{
          position: 'relative',
          display: 'flex',
          width: '340px',
          alignItems: 'center',
          gap: '12px',
          overflow: 'hidden',
          borderRadius: '14px',
          padding: '12px 14px',
          background: 'linear-gradient(135deg, rgba(30,27,50,0.94) 0%, rgba(20,18,38,0.97) 100%)',
          backdropFilter: 'blur(24px)',
          WebkitBackdropFilter: 'blur(24px)',
          border: '1px solid rgba(255,255,255,0.08)',
          boxShadow: `0 16px 48px rgba(0,0,0,0.48), 0 0 60px ${cfg.glow}, inset 0 1px 0 rgba(255,255,255,0.07)`,
        }}
      >
        {/* Ambient glow — contained by overflow:hidden on panel */}
        <div
          style={{
            position: 'absolute',
            top: '-24px',
            left: '-24px',
            width: '96px',
            height: '96px',
            borderRadius: '50%',
            background: cfg.ambient,
            filter: 'blur(20px)',
            pointerEvents: 'none',
          }}
        />

        {/* Top edge highlight */}
        <div
          style={{
            position: 'absolute',
            top: 0,
            left: 0,
            right: 0,
            height: '1px',
            background:
              'linear-gradient(to right, transparent, rgba(255,255,255,0.10), transparent)',
            pointerEvents: 'none',
          }}
        />

        {/* Icon */}
        <div
          style={{
            position: 'relative',
            display: 'flex',
            flexShrink: 0,
            width: '38px',
            height: '38px',
            alignItems: 'center',
            justifyContent: 'center',
            borderRadius: '10px',
            background: cfg.iconBg,
            color: 'var(--color-brand)',
            boxShadow: `0 4px 12px ${cfg.glow}`,
          }}
        >
          <Icon size={18} strokeWidth={2} />
        </div>

        {/* Message */}
        <p
          style={{
            position: 'relative',
            flex: 1,
            margin: 0,
            fontSize: '13px',
            fontWeight: 500,
            lineHeight: 1.4,
            color: 'rgba(255,255,255,0.90)',
          }}
        >
          {item.message}
        </p>

        {/* Close button */}
        <button
          onClick={onClose}
          style={{
            position: 'relative',
            display: 'flex',
            flexShrink: 0,
            width: '28px',
            height: '28px',
            alignItems: 'center',
            justifyContent: 'center',
            borderRadius: '50%',
            background: 'rgba(255,255,255,0.10)',
            color: 'rgba(255,255,255,0.50)',
            border: 'none',
            cursor: 'pointer',
            transition: 'background 150ms, color 150ms',
          }}
          onMouseEnter={(e) => {
            e.currentTarget.style.background = 'rgba(255,255,255,0.18)';
            e.currentTarget.style.color = 'rgba(255,255,255,0.85)';
          }}
          onMouseLeave={(e) => {
            e.currentTarget.style.background = 'rgba(255,255,255,0.10)';
            e.currentTarget.style.color = 'rgba(255,255,255,0.50)';
          }}
        >
          <X size={13} strokeWidth={2.5} />
        </button>
      </div>
    </motion.div>
  );
}

// ─── Portal stack ─────────────────────────────────────────────────────────────

function NotificationStack({
  items,
  dismiss,
}: {
  items: NotificationItem[];
  dismiss: (id: string) => void;
}) {
  return createPortal(
    <div
      style={{
        position: 'fixed',
        bottom: '24px',
        right: '24px',
        display: 'flex',
        flexDirection: 'column-reverse',
        gap: '12px',
        pointerEvents: 'none',
        zIndex: 2147483647,
      }}
    >
      <AnimatePresence initial={false}>
        {items.map((item) => (
          <div key={item.id} style={{ pointerEvents: 'auto' }}>
            <NotificationCard item={item} onClose={() => dismiss(item.id)} />
          </div>
        ))}
      </AnimatePresence>
    </div>,
    document.body
  );
}

// ─── Provider ─────────────────────────────────────────────────────────────────

export function NotificationProvider({ children }: { children: React.ReactNode }) {
  const [items, setItems] = useState<NotificationItem[]>([]);

  const dismiss = useCallback((id: string) => {
    setItems((prev) => prev.filter((n) => n.id !== id));
  }, []);

  const notify = useCallback(
    (message: string, variant: NotificationVariant = 'info', duration = 5000) => {
      const id = `${Date.now()}-${Math.random()}`;
      setItems((prev) => [...prev, { id, message, variant, duration }]);
    },
    []
  );

  return (
    <NotificationContext.Provider value={{ notify }}>
      {children}
      <NotificationStack items={items} dismiss={dismiss} />
    </NotificationContext.Provider>
  );
}

// ─── Hook ─────────────────────────────────────────────────────────────────────

export function useNotification() {
  const ctx = useContext(NotificationContext);
  if (!ctx) throw new Error('useNotification must be used within NotificationProvider');
  return ctx.notify;
}
