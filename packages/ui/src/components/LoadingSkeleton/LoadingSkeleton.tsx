import { cn } from '../../lib/cn';

interface SkeletonProps {
  className?: string;
}

/** Single skeleton line. Compose multiples to match your layout. */
export function Skeleton({ className }: SkeletonProps) {
  return <div className={cn('animate-skeleton rounded-lg bg-white/[0.06]', className)} />;
}

/** Pre-composed card skeleton — mirrors GlassCard content layout. */
export function CardSkeleton({ className }: SkeletonProps) {
  return (
    <div className={cn('glass-card p-5 space-y-3', className)}>
      <div className="flex items-center gap-3">
        <Skeleton className="h-8 w-8 rounded-lg" />
        <Skeleton className="h-4 w-32" />
      </div>
      <Skeleton className="h-3 w-full" />
      <Skeleton className="h-3 w-4/5" />
      <Skeleton className="h-3 w-3/5" />
    </div>
  );
}

/** Pre-composed list-row skeleton. */
export function RowSkeleton({ className }: SkeletonProps) {
  return (
    <div
      className={cn('flex items-center gap-3 rounded-lg bg-white/[0.03] px-3 py-2.5', className)}
    >
      <Skeleton className="h-8 w-8 shrink-0 rounded-full" />
      <div className="flex-1 space-y-1.5">
        <Skeleton className="h-3 w-2/3" />
        <Skeleton className="h-2.5 w-1/3" />
      </div>
    </div>
  );
}
