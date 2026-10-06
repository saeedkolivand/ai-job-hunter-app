import { AnimatePresence } from 'motion/react';
import {
  createContext,
  type ReactNode,
  useCallback,
  useContext,
  useMemo,
  useRef,
  useState,
} from 'react';
import { createPortal } from 'react-dom';

import { NotificationCard } from './NotificationCard';
import { containerStyle, PLACEMENTS } from './placement';
import type {
  NotificationApi,
  NotificationConfig,
  NotificationItem,
  NotificationPlacement,
} from './types';

export type {
  NotificationApi,
  NotificationConfig,
  NotificationPlacement,
  NotificationVariant,
} from './types';

const NotificationContext = createContext<NotificationApi | null>(null);

const DEFAULT_DURATION = 4.5; // seconds
const DEFAULT_PLACEMENT: NotificationPlacement = 'topRight';

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

/** Imperative notification API. Must be used within {@link NotificationProvider}. */
export function useNotification(): NotificationApi {
  const ctx = useContext(NotificationContext);
  if (!ctx) throw new Error('useNotification must be used within NotificationProvider');
  return ctx;
}
