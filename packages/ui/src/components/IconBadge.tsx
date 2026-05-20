import type { LucideIcon } from 'lucide-react';
import type { ElementType } from 'react';
import { cn } from '../lib/cn';

type BadgeSize = 'xs' | 'sm' | 'md' | 'lg';
type BadgeShape = 'rounded' | 'circle' | 'square';

export interface IconBadgeProps {
  icon: LucideIcon | ElementType;
  size?: BadgeSize;
  shape?: BadgeShape;
  className?: string;
  iconClassName?: string;
}

const sizeConfig: Record<BadgeSize, { wrapper: string; icon: number }> = {
  xs: { wrapper: 'h-5 w-5', icon: 10 },
  sm: { wrapper: 'h-7 w-7', icon: 13 },
  md: { wrapper: 'h-8 w-8', icon: 15 },
  lg: { wrapper: 'h-10 w-10', icon: 18 },
};

const shapeClass: Record<BadgeShape, string> = {
  rounded: 'rounded-lg',
  circle: 'rounded-full',
  square: 'rounded-md',
};

export function IconBadge({
  icon: Icon,
  size = 'md',
  shape = 'rounded',
  className,
  iconClassName,
}: IconBadgeProps) {
  const { wrapper, icon } = sizeConfig[size];
  return (
    <div
      className={cn(
        'flex shrink-0 items-center justify-center bg-brand/15',
        wrapper,
        shapeClass[shape],
        className
      )}
    >
      <Icon size={icon} className={cn('text-brand-soft', iconClassName)} />
    </div>
  );
}
