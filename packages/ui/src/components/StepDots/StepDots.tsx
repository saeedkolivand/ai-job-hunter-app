import { cn } from '../../lib/cn';

interface StepDotsProps {
  currentStep: number;
  totalSteps: number;
  /** Wrapper classes — overrides the default centered row + vertical margin. */
  className?: string;
}

export function StepDots({ currentStep, totalSteps, className }: StepDotsProps) {
  return (
    <div className={cn('mb-6 mt-6 flex justify-center gap-1.5', className)}>
      {Array.from({ length: totalSteps }).map((_, i) => (
        <div
          key={i}
          className={`h-1 rounded-full transition-all duration-300 ${
            i === currentStep ? 'w-5 bg-brand' : 'w-1.5 bg-foreground/20'
          }`}
        />
      ))}
    </div>
  );
}
