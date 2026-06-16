import { X } from 'lucide-react';
import {
  type CSSProperties,
  type MouseEvent as ReactMouseEvent,
  type ReactNode,
  useState,
} from 'react';

import { cn } from '../../lib/cn';

// ─── Types ──────────────────────────────────────────────────────────────────

/** antd's named preset hues. */
export type TagPresetColor =
  | 'magenta'
  | 'red'
  | 'volcano'
  | 'orange'
  | 'gold'
  | 'lime'
  | 'green'
  | 'cyan'
  | 'blue'
  | 'geekblue'
  | 'purple';

/** antd's semantic status colours. */
export type TagStatusColor = 'success' | 'processing' | 'error' | 'warning' | 'default';

/** A preset/status name, or any custom CSS colour (rendered as a solid fill). */
export type TagColor = TagPresetColor | TagStatusColor | (string & {});

export interface TagProps {
  /** Preset/status name, or a custom CSS colour string for a solid fill. */
  color?: TagColor;
  /** Leading icon. */
  icon?: ReactNode;
  /** Show a close (×) button. */
  closable?: boolean;
  /** Custom close icon (when `closable`). */
  closeIcon?: ReactNode;
  /** Fired on close click; call `preventDefault()` to keep the tag mounted. */
  onClose?: (e: ReactMouseEvent<HTMLButtonElement>) => void;
  /** Draw the 1px border. Default `true`. */
  bordered?: boolean;
  onClick?: (e: ReactMouseEvent<HTMLSpanElement>) => void;
  children?: ReactNode;
  className?: string;
  style?: CSSProperties;
}

// ─── Colour map ───────────────────────────────────────────────────────────────
// Static class strings (Tailwind-safe — no dynamic concatenation). The palette
// steps (…-400) are remapped to their deeper 600/700 in light mode by tokens.css,
// so every tag stays legible on both light and dark canvases without `dark:`.

const COLOR_CLASS: Record<TagPresetColor | TagStatusColor, string> = {
  // presets
  magenta: 'border-fuchsia-400/30 bg-fuchsia-400/10 text-fuchsia-400',
  red: 'border-red-400/30 bg-red-400/10 text-red-400',
  volcano: 'border-orange-400/30 bg-orange-400/10 text-orange-400',
  orange: 'border-amber-400/30 bg-amber-400/10 text-amber-400',
  gold: 'border-yellow-400/30 bg-yellow-400/10 text-yellow-400',
  lime: 'border-lime-400/30 bg-lime-400/10 text-lime-400',
  green: 'border-green-400/30 bg-green-400/10 text-green-400',
  cyan: 'border-cyan-400/30 bg-cyan-400/10 text-cyan-400',
  blue: 'border-blue-400/30 bg-blue-400/10 text-blue-400',
  geekblue: 'border-indigo-400/30 bg-indigo-400/10 text-indigo-400',
  purple: 'border-purple-400/30 bg-purple-400/10 text-purple-400',
  // status
  success: 'border-emerald-400/30 bg-emerald-400/10 text-emerald-400',
  processing: 'border-blue-400/30 bg-blue-400/10 text-blue-400',
  error: 'border-red-400/30 bg-red-400/10 text-red-400',
  warning: 'border-amber-400/30 bg-amber-400/10 text-amber-400',
  default: 'border-foreground/15 bg-foreground/[0.06] text-foreground/70',
};

const NAMED_COLORS = new Set(Object.keys(COLOR_CLASS));

const BASE =
  'inline-flex items-center gap-1 whitespace-nowrap rounded-md border px-2 py-0.5 text-xs font-medium leading-5 transition-colors';

// ─── Component ──────────────────────────────────────────────────────────────

/**
 * Compact label/tag — an antd-style `Tag`. Named preset/status `color`s render as
 * a tinted chip (theme-safe in light + dark via the palette remap); any other CSS
 * colour string renders as a solid fill with white text. Supports a leading
 * `icon`, an optional `closable` × button, and `bordered={false}`. For a
 * selectable toggle use {@link CheckableTag} (`Tag.CheckableTag`).
 */
function TagBase({
  color,
  icon,
  closable,
  closeIcon,
  onClose,
  bordered = true,
  onClick,
  children,
  className,
  style,
}: TagProps) {
  const [visible, setVisible] = useState(true);
  if (!visible) return null;

  const named = typeof color === 'string' && NAMED_COLORS.has(color);
  const custom = typeof color === 'string' && !named;
  const colorClass = named
    ? COLOR_CLASS[color as TagPresetColor | TagStatusColor]
    : color
      ? '' // custom solid colour handled via inline style
      : COLOR_CLASS.default;
  const customStyle: CSSProperties = custom
    ? { backgroundColor: color, borderColor: color, color: '#fff' }
    : {};

  const handleClose = (e: ReactMouseEvent<HTMLButtonElement>) => {
    e.stopPropagation();
    onClose?.(e);
    if (!e.defaultPrevented) setVisible(false);
  };

  return (
    <span
      className={cn(
        BASE,
        colorClass,
        !bordered && 'border-transparent',
        onClick && 'cursor-pointer',
        className
      )}
      style={{ ...customStyle, ...style }}
      onClick={onClick}
    >
      {icon != null && <span className="-ml-0.5 inline-flex items-center">{icon}</span>}
      {children}
      {closable && (
        <button
          type="button"
          aria-label="Close"
          onClick={handleClose}
          className="-mr-0.5 inline-flex items-center opacity-60 transition-opacity hover:opacity-100"
        >
          {closeIcon ?? <X size={11} strokeWidth={2.5} />}
        </button>
      )}
    </span>
  );
}

export interface CheckableTagProps {
  checked: boolean;
  onChange?: (checked: boolean) => void;
  disabled?: boolean;
  onClick?: (e: ReactMouseEvent<HTMLButtonElement>) => void;
  children?: ReactNode;
  className?: string;
  style?: CSSProperties;
}

/**
 * Selectable tag — toggles `checked` on click (antd's `Tag.CheckableTag`). Renders
 * as a `button` with `aria-pressed`; checked uses the brand accent, unchecked a
 * neutral theme-safe fill. Use for multi-select / filter chips.
 */
function CheckableTag({
  checked,
  onChange,
  disabled,
  onClick,
  children,
  className,
  style,
}: CheckableTagProps) {
  return (
    <button
      type="button"
      aria-pressed={checked}
      disabled={disabled}
      onClick={(e) => {
        onChange?.(!checked);
        onClick?.(e);
      }}
      style={style}
      className={cn(
        BASE,
        'cursor-pointer disabled:cursor-not-allowed disabled:opacity-40',
        checked
          ? 'border-brand/40 bg-brand/15 text-brand-soft'
          : 'border-foreground/12 bg-foreground/[0.03] text-foreground/55 hover:border-foreground/25 hover:text-foreground/85',
        className
      )}
    >
      {children}
    </button>
  );
}

/** {@link TagBase} with the `CheckableTag` sub-component attached. */
export const Tag = Object.assign(TagBase, { CheckableTag });
