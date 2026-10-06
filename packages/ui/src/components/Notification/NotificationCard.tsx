import { AlertTriangle, CheckCircle2, Info, type LucideIcon, X, XCircle } from 'lucide-react';
import { motion } from 'motion/react';
import { useCallback, useEffect, useRef } from 'react';

import { transition } from '../../lib/motion';
import { slideOffset } from './placement';
import type { NotificationItem, NotificationVariant } from './types';

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

// White glyph reads correctly on every variant's fixed (theme-independent) icon
// background; kept as a constant so it isn't a hex literal inside the style object.
const ICON_GLYPH_COLOR = '#fff';

export function NotificationCard({
  item,
  onClose,
}: {
  item: NotificationItem;
  onClose: () => void;
}) {
  const cfg = VARIANTS[item.variant];
  const Icon = cfg.icon;
  const onCloseRef = useRef(onClose);
  onCloseRef.current = onClose;

  // Auto-dismiss with hover-pause: track the remaining time so a hover pauses
  // rather than restarts the countdown.
  const remainingRef = useRef(item.duration * 1000);
  const startRef = useRef(0);
  const timerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  const clear = useCallback(() => {
    if (timerRef.current) {
      clearTimeout(timerRef.current);
      timerRef.current = null;
    }
  }, []);

  const start = useCallback(() => {
    if (item.duration <= 0 || remainingRef.current <= 0) return;
    startRef.current = Date.now();
    timerRef.current = setTimeout(() => onCloseRef.current(), remainingRef.current);
  }, [item.duration]);

  const pause = useCallback(() => {
    if (!timerRef.current) return;
    clear();
    remainingRef.current -= Date.now() - startRef.current;
  }, [clear]);

  useEffect(() => {
    // Reset + (re)start whenever the content or duration changes (key update).
    remainingRef.current = item.duration * 1000;
    clear();
    start();
    return clear;
  }, [item.key, item.duration, item.message, item.description, clear, start]);

  const offset = slideOffset(item.placement);

  return (
    <motion.div
      layout
      initial={{ opacity: 0, scale: 0.96, x: offset.x, y: offset.y }}
      animate={{ opacity: 1, scale: 1, x: 0, y: 0 }}
      exit={{ opacity: 0, scale: 0.97, x: offset.x, y: offset.y }}
      transition={transition.relaxed}
      style={{ position: 'relative', pointerEvents: 'auto' }}
      onMouseEnter={item.pauseOnHover ? pause : undefined}
      onMouseLeave={item.pauseOnHover ? start : undefined}
      role="alert"
    >
      <div
        style={{
          position: 'relative',
          display: 'flex',
          width: '340px',
          maxWidth: '100%',
          alignItems: 'flex-start',
          gap: '12px',
          overflow: 'hidden',
          borderRadius: '14px',
          padding: '14px',
          // Themed glass surface (charcoal in dark, white in light) via tokens —
          // replaces the hardcoded violet gradient so it fits both schemes.
          background:
            'linear-gradient(135deg, rgb(var(--glass-rgb) / 0.97) 0%, rgb(var(--glass-rgb) / 0.99) 100%)',
          backdropFilter: 'blur(24px)',
          WebkitBackdropFilter: 'blur(24px)',
          border: '1px solid var(--border-mid)',
          boxShadow: 'var(--shadow-xl), var(--glass-specular)',
        }}
      >
        {/* Variant glow washing in from the left, behind the icon (like the
            privacy ActionCard). The top sheen is handled by --glass-specular. */}
        <div
          style={{
            position: 'absolute',
            bottom: '-22px',
            left: '-22px',
            width: '120px',
            height: '120px',
            borderRadius: '50%',
            background: cfg.glow,
            filter: 'blur(36px)',
            pointerEvents: 'none',
          }}
        />

        {/* Icon */}
        {item.icon !== null && (
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
              color: ICON_GLYPH_COLOR,
              boxShadow: `0 4px 12px ${cfg.glow}`,
            }}
          >
            {item.icon ?? <Icon size={18} strokeWidth={2} />}
          </div>
        )}

        {/* Text + actions */}
        <div style={{ position: 'relative', flex: 1, minWidth: 0 }}>
          <p
            style={{
              margin: 0,
              fontSize: '13px',
              fontWeight: 600,
              lineHeight: 1.4,
              color: 'var(--color-foreground)',
            }}
          >
            {item.message}
          </p>
          {item.description != null && (
            <p
              style={{
                margin: '4px 0 0',
                fontSize: '12px',
                fontWeight: 400,
                lineHeight: 1.45,
                color: 'color-mix(in oklab, var(--color-foreground) 60%, transparent)',
              }}
            >
              {item.description}
            </p>
          )}
          {item.btn != null && <div style={{ marginTop: '10px' }}>{item.btn}</div>}
        </div>

        {/* Close button */}
        {item.closable && (
          <button
            type="button"
            aria-label="Close notification"
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
              background: 'color-mix(in oklab, var(--color-foreground) 8%, transparent)',
              color: 'color-mix(in oklab, var(--color-foreground) 45%, transparent)',
              border: 'none',
              cursor: 'pointer',
              transition: 'background 150ms, color 150ms',
            }}
            onMouseEnter={(e) => {
              e.currentTarget.style.background =
                'color-mix(in oklab, var(--color-foreground) 15%, transparent)';
              e.currentTarget.style.color =
                'color-mix(in oklab, var(--color-foreground) 80%, transparent)';
            }}
            onMouseLeave={(e) => {
              e.currentTarget.style.background =
                'color-mix(in oklab, var(--color-foreground) 8%, transparent)';
              e.currentTarget.style.color =
                'color-mix(in oklab, var(--color-foreground) 45%, transparent)';
            }}
          >
            <X size={13} strokeWidth={2.5} />
          </button>
        )}
      </div>
    </motion.div>
  );
}
