import { Briefcase, Globe, type LucideIcon } from 'lucide-react';
import { motion } from 'motion/react';
import type { ReactNode } from 'react';

import { cn } from '../../lib/cn';
import { transition } from '../../lib/motion';

const PLATFORM_CONFIG: Record<string, { icon: LucideIcon; color: string; label: string }> = {
  linkedin: {
    icon: Globe,
    color: '#0A66C2',
    label: 'LinkedIn',
  },
  indeed: {
    icon: Briefcase,
    color: '#2557A7',
    label: 'Indeed',
  },
  xing: {
    icon: Globe,
    color: '#006567',
    label: 'XING',
  },
  glassdoor: {
    icon: Briefcase,
    color: '#0CAA41',
    label: 'Glassdoor',
  },
};

export interface SourceBadgeProps {
  source: string;
  url?: string;
  className?: string;
  children?: ReactNode;
}

export function SourceBadge({ source, url, className, children }: SourceBadgeProps) {
  const config = PLATFORM_CONFIG[source.toLowerCase()] || {
    icon: Globe,
    color: '#6366F1',
    label: source,
  };
  const Icon = config.icon;

  const handleClick = () => {
    if (url) {
      window.open(url, '_blank');
    }
  };

  return (
    <motion.div
      transition={transition.fast}
      className={cn(
        'inline-flex items-center gap-1.5 rounded-full border px-2.5 py-1 text-[11px] font-semibold',
        'backdrop-blur-md transition-all duration-150',
        'hover:shadow-lg',
        className
      )}
      style={{
        backgroundColor: `${config.color}10`,
        borderColor: `${config.color}30`,
        color: config.color,
      }}
      onClick={handleClick}
      title={`Scraped from ${config.label}`}
    >
      <Icon size={11} />
      <span>{config.label}</span>
      {children}
    </motion.div>
  );
}
