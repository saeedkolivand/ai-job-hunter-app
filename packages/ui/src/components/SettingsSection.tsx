import type { LucideIcon } from 'lucide-react';
import type { ElementType, ReactNode } from 'react';

import { GlassCard } from './GlassCard';
import { IconBadge } from './IconBadge';
import { SectionLabel } from './SectionLabel';

interface SettingsSectionProps {
  icon: LucideIcon | ElementType;
  label: string;
  children: ReactNode;
}

/**
 * Consistent settings card with labelled header.
 * Replaces the repeated `GlassCard > div.mb-4 > IconBadge + SectionLabel` pattern.
 */
export function SettingsSection({ icon, label, children }: SettingsSectionProps) {
  return (
    <GlassCard>
      <div className="mb-4 flex items-center gap-2">
        <IconBadge icon={icon} size="sm" />
        <SectionLabel>{label}</SectionLabel>
      </div>
      {children}
    </GlassCard>
  );
}
