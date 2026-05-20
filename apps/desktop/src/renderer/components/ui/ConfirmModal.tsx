import { type LucideIcon, AlertTriangle, X, Info, CheckCircle, AlertOctagon } from 'lucide-react';
import { Button } from './Button';
import { ModalShell } from './ModalShell';
import { cn } from '@/lib/cn';

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
    confirmVariant: NonNullable<React.ComponentProps<typeof Button>['variant']>;
  }
> = {
  danger: {
    icon: AlertOctagon,
    iconBg: 'bg-red-500/20',
    iconColor: 'text-red-400',
    border: 'border-red-500/30',
    confirmVariant: 'danger',
  },
  warning: {
    icon: AlertTriangle,
    iconBg: 'bg-orange-500/20',
    iconColor: 'text-orange-400',
    border: 'border-orange-500/30',
    confirmVariant: 'warning',
  },
  info: {
    icon: Info,
    iconBg: 'bg-blue-500/20',
    iconColor: 'text-blue-400',
    border: 'border-blue-500/30',
    confirmVariant: 'info',
  },
  success: {
    icon: CheckCircle,
    iconBg: 'bg-green-500/20',
    iconColor: 'text-green-400',
    border: 'border-green-500/30',
    confirmVariant: 'success',
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

  return (
    <ModalShell open={open} onClose={onClose} borderClass={config.border} zIndex={600}>
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
          <div className="text-base font-medium text-foreground" id="modal-title">
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
      <div className="flex items-center justify-end gap-3 border-t border-white/5 px-6 py-4">
        <Button variant="ghost" size="md" onClick={onClose} disabled={isConfirming}>
          {cancelText}
        </Button>
        <Button
          variant={config.confirmVariant}
          size="md"
          loading={isConfirming}
          onClick={onConfirm}
        >
          {confirmText}
        </Button>
      </div>
    </ModalShell>
  );
}
