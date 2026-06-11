import { AlertTriangle, CheckCircle2, Info, type LucideIcon, X, XCircle } from 'lucide-react';
import { type CSSProperties, type ReactNode, useState } from 'react';

// ─── Types ────────────────────────────────────────────────────────────────────

export type AlertType = 'success' | 'info' | 'warning' | 'error';

export interface AlertProps {
  /** Main content line. */
  message: ReactNode;
  /** Optional additional detail under the message. */
  description?: ReactNode;
  /** Visual type. Default `info` (or `warning` in `banner` mode when unset). */
  type?: AlertType;
  /** Show the type icon. Default `false` (antd parity); auto-on with a description. */
  showIcon?: boolean;
  /** Override the type icon. */
  icon?: ReactNode;
  /** Show a close button. `true`, or an object with a custom `closeIcon`. */
  closable?: boolean | { closeIcon?: ReactNode };
  /** Fired after the alert is closed. */
  onClose?: () => void;
  /** Action area rendered on the right (e.g. a button). */
  action?: ReactNode;
  /** Banner mode: full-width, square corners (sits flush at the top of a region). */
  banner?: boolean;
  className?: string;
  style?: CSSProperties;
}

// ─── Type config (semantic colours, matched to Notification) ────────────────────

const TYPES: Record<AlertType, { icon: LucideIcon; accent: string; bg: string; border: string }> = {
  success: {
    icon: CheckCircle2,
    accent: '#10b981',
    bg: 'rgba(16,185,129,0.10)',
    border: 'rgba(16,185,129,0.32)',
  },
  info: {
    icon: Info,
    accent: '#3b82f6',
    bg: 'rgba(59,130,246,0.10)',
    border: 'rgba(59,130,246,0.32)',
  },
  warning: {
    icon: AlertTriangle,
    accent: '#f59e0b',
    bg: 'rgba(245,158,11,0.10)',
    border: 'rgba(245,158,11,0.32)',
  },
  error: {
    icon: XCircle,
    accent: '#ef4444',
    bg: 'rgba(239,68,68,0.10)',
    border: 'rgba(239,68,68,0.32)',
  },
};

// ─── Component ──────────────────────────────────────────────────────────────────

/**
 * Inline status banner, modelled on antd's `Alert`. Unlike `useNotification`
 * (transient, floating, imperative) this is a persistent element you place in the
 * layout. Supports `message` + optional `description`, a type icon, a close
 * button, an `action` slot, and `banner` mode.
 */
export function Alert({
  message,
  description,
  type,
  showIcon,
  icon,
  closable,
  onClose,
  action,
  banner = false,
  className,
  style,
}: AlertProps) {
  const [closed, setClosed] = useState(false);
  if (closed) return null;

  const resolvedType: AlertType = type ?? (banner ? 'warning' : 'info');
  const cfg = TYPES[resolvedType];
  const Icon = cfg.icon;
  const withIcon = showIcon ?? description != null; // antd: icon emphasised in description mode
  const isClosable = closable === true || (typeof closable === 'object' && closable !== null);
  const closeIcon =
    typeof closable === 'object' && closable?.closeIcon ? (
      closable.closeIcon
    ) : (
      <X size={14} strokeWidth={2.5} />
    );

  const close = () => {
    setClosed(true);
    onClose?.();
  };

  return (
    <div
      role="alert"
      className={className}
      style={{
        display: 'flex',
        alignItems: description != null ? 'flex-start' : 'center',
        gap: '10px',
        padding: description != null ? '12px 14px' : '8px 12px',
        background: cfg.bg,
        border: `1px solid ${cfg.border}`,
        borderRadius: banner ? 0 : '10px',
        color: 'rgba(255,255,255,0.88)',
        fontSize: '13px',
        lineHeight: 1.45,
        ...style,
      }}
    >
      {withIcon && (
        <span
          style={{
            display: 'flex',
            flexShrink: 0,
            alignItems: 'center',
            color: cfg.accent,
            marginTop: description != null ? '1px' : 0,
          }}
        >
          {icon ?? <Icon size={description != null ? 18 : 16} strokeWidth={2.2} />}
        </span>
      )}

      <div style={{ flex: 1, minWidth: 0 }}>
        <div
          style={{ fontWeight: description != null ? 600 : 500, color: 'rgba(255,255,255,0.92)' }}
        >
          {message}
        </div>
        {description != null && (
          <div style={{ marginTop: '3px', fontSize: '12px', color: 'rgba(255,255,255,0.62)' }}>
            {description}
          </div>
        )}
      </div>

      {action != null && <div style={{ flexShrink: 0 }}>{action}</div>}

      {isClosable && (
        <button
          type="button"
          aria-label="Close alert"
          onClick={close}
          style={{
            display: 'flex',
            flexShrink: 0,
            alignItems: 'center',
            justifyContent: 'center',
            padding: '2px',
            background: 'none',
            border: 'none',
            cursor: 'pointer',
            color: 'rgba(255,255,255,0.45)',
            transition: 'color 150ms',
          }}
          onMouseEnter={(e) => (e.currentTarget.style.color = 'rgba(255,255,255,0.85)')}
          onMouseLeave={(e) => (e.currentTarget.style.color = 'rgba(255,255,255,0.45)')}
        >
          {closeIcon}
        </button>
      )}
    </div>
  );
}
