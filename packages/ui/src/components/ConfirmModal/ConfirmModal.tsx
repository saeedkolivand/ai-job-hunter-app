import { AlertOctagon, AlertTriangle, CheckCircle, Info, type LucideIcon, X } from 'lucide-react';
import { useId } from 'react';

import { cn } from '../../lib/cn';
import { Button } from '../Button';
import { ModalShell } from '../ModalShell';

interface ConfirmModalProps {
  open: boolean;
  onClose: () => void;
  onConfirm: () => void;
  title: string;
  description: string;
  confirmText?: string;
  cancelText?: string;
  variant?: 'danger' | 'warning' | 'info' | 'success';
  isConfirming?: boolean;
}

const variantConfig: Record<
  NonNullable<ConfirmModalProps['variant']>,
  {
    icon: LucideIcon;
    iconBg: string;
    iconColor: string;
    border: string;
    confirmClass: string;
    glow: string;
  }
> = {
  danger: {
    icon: AlertOctagon,
    iconBg: 'bg-red-500/20',
    iconColor: 'text-red-400',
    border: 'border-red-500/30',
    confirmClass: 'border-red-500/50 text-red-400 hover:border-red-500/70 hover:text-red-300',
    glow: '0 0 16px rgba(239,68,68,0.25)',
  },
  warning: {
    icon: AlertTriangle,
    iconBg: 'bg-orange-500/20',
    iconColor: 'text-orange-400',
    border: 'border-orange-500/30',
    confirmClass:
      'border-amber-500/50 text-amber-400 hover:border-amber-500/70 hover:text-amber-300',
    glow: '0 0 16px rgba(245,158,11,0.25)',
  },
  info: {
    icon: Info,
    iconBg: 'bg-blue-500/20',
    iconColor: 'text-blue-400',
    border: 'border-blue-500/30',
    confirmClass: 'border-blue-500/50 text-blue-400 hover:border-blue-500/70 hover:text-blue-300',
    glow: '0 0 16px rgba(59,130,246,0.25)',
  },
  success: {
    icon: CheckCircle,
    iconBg: 'bg-green-500/20',
    iconColor: 'text-green-400',
    border: 'border-green-500/30',
    confirmClass:
      'border-emerald-500/50 text-emerald-400 hover:border-emerald-500/70 hover:text-emerald-300',
    glow: '0 0 16px rgba(16,185,129,0.25)',
  },
};

export function ConfirmModal({
  open,
  onClose,
  onConfirm,
  title,
  description,
  confirmText = 'Confirm',
  cancelText = 'Cancel',
  variant = 'info',
  isConfirming = false,
}: ConfirmModalProps) {
  const config = variantConfig[variant];
  const Icon = config.icon;
  const titleId = useId();

  return (
    <ModalShell
      open={open}
      onClose={onClose}
      borderClass={config.border}
      zIndex={600}
      ariaLabelledby={titleId}
    >
      {/* Header */}
      <div className="flex items-start justify-between border-b border-white/5 px-6 py-5">
        <div className="flex items-center gap-3">
          <div
            className={cn(
              'flex h-9 w-9 items-center justify-center rounded-xl',
              config.iconBg,
              config.iconColor
            )}
          >
            <Icon size={18} aria-hidden="true" />
          </div>
          <div className="text-base font-medium text-foreground" id={titleId}>
            {title}
          </div>
        </div>
        <button
          onClick={onClose}
          aria-label="Close dialog"
          className="rounded-xl bg-white/5 p-1.5 text-foreground/60 transition-all duration-150 hover:bg-white/10 hover:text-foreground"
        >
          <X size={14} aria-hidden="true" />
        </button>
      </div>

      {/* Body */}
      <div className="px-6 py-5">
        <p className="text-sm leading-relaxed text-foreground/70">{description}</p>
      </div>

      {/* Footer */}
      <div className="flex items-center justify-end gap-2 border-t border-white/5 px-6 py-4">
        <Button
          variant="ghost"
          size="md"
          onClick={onClose}
          disabled={isConfirming}
          className="px-5"
        >
          {cancelText}
        </Button>
        <button
          disabled={isConfirming}
          onClick={onConfirm}
          style={{ boxShadow: isConfirming ? 'none' : config.glow }}
          className={cn(
            'inline-flex h-8 items-center gap-2 rounded-lg border bg-transparent px-5 text-sm font-medium transition-all duration-150',
            'disabled:pointer-events-none disabled:opacity-45',
            config.confirmClass
          )}
        >
          {isConfirming && (
            <span className="h-3 w-3 animate-spin rounded-full border-[1.5px] border-current border-t-transparent" />
          )}
          {confirmText}
        </button>
      </div>
    </ModalShell>
  );
}
