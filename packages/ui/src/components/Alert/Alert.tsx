import { AlertTriangle, CheckCircle2, Info, type LucideIcon, X, XCircle } from 'lucide-react';
import { type CSSProperties, type ReactNode, useState } from 'react';

import { cn } from '../../lib/cn';
import { Button } from '../Button';

// ─── Types ────────────────────────────────────────────────────────────────────

export type AlertType = 'success' | 'info' | 'warning' | 'error';

export interface AlertProps {
  /** Main content line. */
  message: ReactNode;
  /** Optional additional detail under the message. */
  description?: ReactNode;
  /** Visual type. Default `info` (or `warning` in `banner` mode when unset). */
  type?: AlertType;
  /** Show the type icon. Default `false`; auto-on with a description. */
  showIcon?: boolean;
  /** Override the type icon. */
  icon?: ReactNode;
  /** Show a close button. `true`, or an object with a custom `closeIcon`. */
  closable?: boolean | { closeIcon?: ReactNode };
  /** Fired after the alert is closed. */
  onClose?: () => void;
  /** Accessible label for the close button. Defaults to 'Close alert'. */
  closeAriaLabel?: string;
  /** Action area rendered on the right (e.g. a button). */
  action?: ReactNode;
  /** Banner mode: full-width, square corners (sits flush at the top of a region). */
  banner?: boolean;
  className?: string;
  style?: CSSProperties;
}

// ─── Type config (Tailwind palette classes — same tokens used across the app) ─

const TYPES: Record<
  AlertType,
  { icon: LucideIcon; iconClass: string; bgClass: string; borderClass: string }
> = {
  success: {
    icon: CheckCircle2,
    iconClass: 'text-emerald-400',
    bgClass: 'bg-emerald-500/10',
    borderClass: 'border-emerald-500/30',
  },
  info: {
    icon: Info,
    iconClass: 'text-blue-400',
    bgClass: 'bg-blue-500/10',
    borderClass: 'border-blue-500/30',
  },
  warning: {
    icon: AlertTriangle,
    iconClass: 'text-amber-400',
    bgClass: 'bg-amber-500/10',
    borderClass: 'border-amber-500/30',
  },
  error: {
    icon: XCircle,
    iconClass: 'text-red-400',
    bgClass: 'bg-red-500/10',
    borderClass: 'border-red-500/30',
  },
};

// ─── Component ──────────────────────────────────────────────────────────────────

/**
 * Inline status banner. Unlike `useNotification`
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
  closeAriaLabel = 'Close alert',
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
  const withIcon = showIcon ?? description != null; // icon emphasised in description mode
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
      style={style}
      className={cn(
        'flex gap-[10px] border text-[13px] leading-[1.45] text-foreground/80',
        description != null ? 'items-start px-[14px] py-3' : 'items-center px-3 py-2',
        banner ? 'rounded-none' : 'rounded-[10px]',
        cfg.bgClass,
        cfg.borderClass,
        className
      )}
    >
      {withIcon && (
        <span
          className={cn(
            'flex shrink-0 items-center',
            cfg.iconClass,
            description != null && 'mt-[1px]'
          )}
        >
          {icon ?? <Icon size={description != null ? 18 : 16} strokeWidth={2.2} />}
        </span>
      )}

      <div className="flex-1 min-w-0 text-foreground/90">
        <div className={description != null ? 'font-semibold' : 'font-normal'}>{message}</div>
        {description != null && (
          <div className="mt-[3px] text-[12px] text-foreground/60">{description}</div>
        )}
      </div>

      {action != null && <div className="shrink-0">{action}</div>}

      {isClosable && (
        <Button
          variant="unstyled"
          type="button"
          aria-label={closeAriaLabel}
          onClick={close}
          className="flex shrink-0 items-center justify-center p-[2px] text-foreground/45 transition-colors duration-150 hover:text-foreground/85"
        >
          {closeIcon}
        </Button>
      )}
    </div>
  );
}
