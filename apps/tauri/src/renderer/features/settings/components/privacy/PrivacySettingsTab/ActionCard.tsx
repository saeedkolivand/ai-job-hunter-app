import { Button, cn } from '@ajh/ui';

export interface ActionCardProps {
  icon: React.ElementType;
  iconBg: string;
  iconColor: string;
  glowColor: string;
  title: string;
  description: string;
  buttonLabel: string;
  buttonBorder: string;
  buttonText: string;
  buttonGlow: string;
  loading?: boolean;
  onClick: () => void;
}

export function ActionCard({
  icon: Icon,
  iconBg,
  iconColor,
  glowColor,
  title,
  description,
  buttonLabel,
  buttonBorder,
  buttonText,
  buttonGlow,
  loading,
  onClick,
}: ActionCardProps) {
  return (
    <div
      className="relative flex items-center gap-4 overflow-hidden rounded-xl border border-white/[0.07] px-4 py-3.5"
      style={{
        background:
          'linear-gradient(135deg, rgba(255,255,255,0.03) 0%, rgba(255,255,255,0.01) 100%)',
      }}
    >
      {/* Ambient glow behind icon */}
      <div
        className="pointer-events-none absolute -bottom-4 -left-4 h-24 w-24 rounded-full blur-2xl"
        style={{ background: glowColor }}
      />

      {/* Icon */}
      <div
        className={cn(
          'relative flex h-10 w-10 shrink-0 items-center justify-center rounded-xl shadow-md',
          iconBg
        )}
      >
        <Icon size={18} className={iconColor} strokeWidth={1.75} />
      </div>

      {/* Text */}
      <div className="relative min-w-0 flex-1">
        <div className="text-sm font-semibold text-white/90">{title}</div>
        <div className="text-xs text-white/40 leading-snug mt-0.5">{description}</div>
      </div>

      {/* Outlined action button */}
      <Button
        onClick={onClick}
        disabled={loading}
        className={cn(
          'relative shrink-0 rounded-lg border px-3 py-1.5 text-xs font-medium transition-all duration-150 h-auto',
          'disabled:pointer-events-none disabled:opacity-40',
          buttonBorder,
          buttonText
        )}
        style={{ boxShadow: loading ? 'none' : buttonGlow }}
      >
        {loading ? (
          <>
            <span className="h-3 w-3 animate-spin rounded-full border-[1.5px] border-current border-t-transparent" />
            {buttonLabel}
          </>
        ) : (
          buttonLabel
        )}
      </Button>
    </div>
  );
}
