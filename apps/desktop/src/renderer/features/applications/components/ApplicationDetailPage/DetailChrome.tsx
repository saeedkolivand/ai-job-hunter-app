import { ArrowLeft } from 'lucide-react';

import { Button } from '@ajh/ui';

/** Slim header + bordered-panel chrome shared by loading / error / loaded states. */
export function SlimLayout({
  onBack,
  backLabel,
  title,
  children,
}: {
  onBack: () => void;
  backLabel: string;
  title: string;
  children: React.ReactNode;
}) {
  return (
    <div className="flex h-full flex-col">
      <div className="flex shrink-0 items-center gap-3 border-b border-[var(--border-soft)] px-8 py-4">
        <BackButton onBack={onBack} backLabel={backLabel} />
        <div className="min-w-0 flex-1">
          <span className="truncate text-base font-semibold text-foreground/90">{title}</span>
        </div>
      </div>
      <div className="min-h-0 flex-1 p-4">{children}</div>
    </div>
  );
}

export function BackButton({ onBack, backLabel }: { onBack: () => void; backLabel: string }) {
  return (
    <Button
      onClick={onBack}
      variant="ghost"
      className="shrink-0 gap-1.5 text-foreground/50 hover:text-foreground/80"
    >
      <ArrowLeft size={14} /> {backLabel}
    </Button>
  );
}

/** The bordered tabbed-panel surface (fills its parent height). */
export function PanelShell({ children }: { children: React.ReactNode }) {
  return (
    <div className="flex h-full min-h-0 flex-col overflow-hidden rounded-lg border border-[var(--border-soft)] bg-card">
      {children}
    </div>
  );
}

/** Scroll + padding wrapper for the prose tabs (Timeline / Brief). */
export function TabScroll({ children }: { children: React.ReactNode }) {
  return <div className="h-full space-y-4 overflow-y-auto px-6 py-5">{children}</div>;
}

export function FieldLabel({ htmlFor, children }: { htmlFor: string; children: string }) {
  return (
    <label htmlFor={htmlFor} className="text-xs font-semibold text-foreground/70">
      {children}
    </label>
  );
}

export function FieldError({ children }: { children: string }) {
  return (
    <p className="text-fine-print text-red-400" role="alert">
      {children}
    </p>
  );
}
