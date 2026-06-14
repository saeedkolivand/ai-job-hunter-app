import { AlertTriangle, CheckCircle2, Info, type LucideIcon, X, XCircle } from 'lucide-react';
import { AnimatePresence, motion } from 'motion/react';
import {
  createContext,
  type CSSProperties,
  type ReactNode,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useRef,
  useState,
} from 'react';
import { createPortal } from 'react-dom';

import { transition } from '../../lib/motion';

// ─── Types ────────────────────────────────────────────────────────────────────

export type NotificationVariant = 'success' | 'error' | 'info' | 'warning';

/** All six corner/edge placements, matching antd's `notification`. */
export type NotificationPlacement =
  | 'top'
  | 'topLeft'
  | 'topRight'
  | 'bottom'
  | 'bottomLeft'
  | 'bottomRight';

/** Open a notification. Modelled on antd's `notification` config. `duration` is
 *  in SECONDS (antd convention); `0` keeps it open until dismissed. */
export interface NotificationConfig {
  /** Title line (bold). */
  message: ReactNode;
  /** Optional secondary line. */
  description?: ReactNode;
  variant?: NotificationVariant;
  /** Seconds before auto-dismiss. `0` = sticky. Default `4.5`. */
  duration?: number;
  /** Corner/edge to render in. Default `topRight`. */
  placement?: NotificationPlacement;
  /** Custom action area rendered under the text (e.g. a button). */
  btn?: ReactNode;
  /** Override the variant icon. Pass `null` to hide it. */
  icon?: ReactNode;
  /** Stable id — opening with an existing key UPDATES that notification (and
   *  resets its timer) instead of stacking a duplicate. Auto-generated if absent. */
  key?: string;
  /** Show the close button. Default `true`. */
  closable?: boolean;
  /** Pause the auto-dismiss timer while hovered. Default `true`. */
  pauseOnHover?: boolean;
  onClose?: () => void;
}

/** Imperative API returned by {@link useNotification}. Mirrors antd. */
export interface NotificationApi {
  open: (config: NotificationConfig) => string;
  success: (config: Omit<NotificationConfig, 'variant'>) => string;
  error: (config: Omit<NotificationConfig, 'variant'>) => string;
  info: (config: Omit<NotificationConfig, 'variant'>) => string;
  warning: (config: Omit<NotificationConfig, 'variant'>) => string;
  /** Dismiss one notification by key, or all when called with no key. */
  destroy: (key?: string) => void;
}

interface NotificationItem {
  key: string;
  message: ReactNode;
  description?: ReactNode;
  variant: NotificationVariant;
  duration: number;
  placement: NotificationPlacement;
  btn?: ReactNode;
  icon?: ReactNode;
  closable: boolean;
  pauseOnHover: boolean;
  onClose?: () => void;
}

const NotificationContext = createContext<NotificationApi | null>(null);

const DEFAULT_DURATION = 4.5; // seconds (antd default)
const DEFAULT_PLACEMENT: NotificationPlacement = 'topRight';

const PLACEMENTS: NotificationPlacement[] = [
  'top',
  'topLeft',
  'topRight',
  'bottom',
  'bottomLeft',
  'bottomRight',
];

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

// White glyph reads correctly on every variant's fixed (theme-independent) icon
// background; kept as a constant so it isn't a hex literal inside the style object.
const ICON_GLYPH_COLOR = '#fff';

// ─── Placement geometry ─────────────────────────────────────────────────────

/** Fixed-container anchor for a placement. */
function containerStyle(placement: NotificationPlacement): CSSProperties {
  const base: CSSProperties = {
    position: 'fixed',
    display: 'flex',
    flexDirection: 'column',
    gap: '12px',
    pointerEvents: 'none',
    zIndex: 2147483647,
    maxWidth: 'calc(100vw - 48px)',
  };
  const top = placement.startsWith('top');
  const vertical: CSSProperties = top ? { top: '24px' } : { bottom: '24px' };
  let horizontal: CSSProperties;
  let align: CSSProperties['alignItems'];
  if (placement.endsWith('Left')) {
    horizontal = { left: '24px' };
    align = 'flex-start';
  } else if (placement.endsWith('Right')) {
    horizontal = { right: '24px' };
    align = 'flex-end';
  } else {
    horizontal = { left: '50%', transform: 'translateX(-50%)' };
    align = 'center';
  }
  return { ...base, ...vertical, ...horizontal, alignItems: align };
}

/** Enter/exit offset so each placement slides in from its own edge. */
function slideOffset(placement: NotificationPlacement): { x: number; y: number } {
  if (placement.endsWith('Right')) return { x: 40, y: 0 };
  if (placement.endsWith('Left')) return { x: -40, y: 0 };
  return { x: 0, y: placement.startsWith('top') ? -40 : 40 };
}

// ─── Card ─────────────────────────────────────────────────────────────────────

function NotificationCard({ item, onClose }: { item: NotificationItem; onClose: () => void }) {
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

// ─── Portal stack (one per placement) ──────────────────────────────────────────

function NotificationStacks({
  items,
  dismiss,
}: {
  items: NotificationItem[];
  dismiss: (key: string) => void;
}) {
  return createPortal(
    <>
      {PLACEMENTS.map((placement) => {
        const group = items.filter((n) => n.placement === placement);
        if (group.length === 0) return null;
        // Top placements show newest on top; bottom placements newest at the
        // bottom (nearest the anchored corner).
        const ordered = placement.startsWith('top') ? [...group].reverse() : group;
        return (
          <div key={placement} style={containerStyle(placement)}>
            <AnimatePresence>
              {ordered.map((item) => (
                <NotificationCard key={item.key} item={item} onClose={() => dismiss(item.key)} />
              ))}
            </AnimatePresence>
          </div>
        );
      })}
    </>,
    document.body
  );
}

// ─── Provider ─────────────────────────────────────────────────────────────────

export function NotificationProvider({ children }: { children: ReactNode }) {
  const [items, setItems] = useState<NotificationItem[]>([]);
  const keySeq = useRef(0);

  const dismiss = useCallback((key: string) => {
    setItems((prev) => {
      const hit = prev.find((n) => n.key === key);
      hit?.onClose?.();
      return prev.filter((n) => n.key !== key);
    });
  }, []);

  const destroy = useCallback(
    (key?: string) => {
      if (key == null) {
        setItems((prev) => {
          prev.forEach((n) => n.onClose?.());
          return [];
        });
      } else {
        dismiss(key);
      }
    },
    [dismiss]
  );

  const open = useCallback((config: NotificationConfig): string => {
    const key = config.key ?? `ntf-${Date.now()}-${(keySeq.current += 1)}`;
    const item: NotificationItem = {
      key,
      message: config.message,
      description: config.description,
      variant: config.variant ?? 'info',
      duration: config.duration ?? DEFAULT_DURATION,
      placement: config.placement ?? DEFAULT_PLACEMENT,
      btn: config.btn,
      icon: config.icon,
      closable: config.closable ?? true,
      pauseOnHover: config.pauseOnHover ?? true,
      onClose: config.onClose,
    };
    setItems((prev) => {
      const idx = prev.findIndex((n) => n.key === key);
      if (idx === -1) return [...prev, item];
      // Update-in-place (same key) — keeps stack position, resets via card effect.
      const next = [...prev];
      next[idx] = item;
      return next;
    });
    return key;
  }, []);

  const api = useMemo<NotificationApi>(
    () => ({
      open,
      success: (c) => open({ ...c, variant: 'success' }),
      error: (c) => open({ ...c, variant: 'error' }),
      info: (c) => open({ ...c, variant: 'info' }),
      warning: (c) => open({ ...c, variant: 'warning' }),
      destroy,
    }),
    [open, destroy]
  );

  return (
    <NotificationContext.Provider value={api}>
      {children}
      <NotificationStacks items={items} dismiss={dismiss} />
    </NotificationContext.Provider>
  );
}

// ─── Hook ─────────────────────────────────────────────────────────────────────

/** Imperative notification API (antd-style). Must be used within
 *  {@link NotificationProvider}. */
export function useNotification(): NotificationApi {
  const ctx = useContext(NotificationContext);
  if (!ctx) throw new Error('useNotification must be used within NotificationProvider');
  return ctx;
}
