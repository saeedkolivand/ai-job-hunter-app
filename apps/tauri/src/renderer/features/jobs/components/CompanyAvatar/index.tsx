import { cn } from '@ajh/ui';

interface CompanyAvatarProps {
  company: string;
  sourceFallback?: string;
  size?: 'sm' | 'md';
}

// ponytail: 7 token pairs — brand, teal, blue, amber, rose, violet, emerald
// charCodeAt(0) % 7 maps company initials deterministically to a slot.
const SLOTS = [
  'bg-brand/10 text-brand ring-brand/20', // violet
  'bg-teal-500/10 text-teal-600 ring-teal-500/20 dark:text-teal-400', // teal
  'bg-blue-500/10 text-blue-600 ring-blue-500/20 dark:text-blue-400', // blue
  'bg-amber-500/10 text-amber-600 ring-amber-500/20 dark:text-amber-400', // amber
  'bg-rose-500/10 text-rose-600 ring-rose-500/20 dark:text-rose-400', // rose
  'bg-violet-500/10 text-violet-600 ring-violet-500/20 dark:text-violet-400', // violet-2
  'bg-emerald-500/10 text-emerald-600 ring-emerald-500/20 dark:text-emerald-400', // emerald
] as const;

function initials(name: string): string {
  const trimmed = name.trim();
  if (!trimmed) return '?';
  const words = trimmed.split(/\s+/);
  if (words.length === 1) {
    // ponytail: single word → take first 2 chars
    return trimmed.slice(0, 2).toUpperCase();
  }
  // Multi-word → first letter of first + first letter of last
  const first = words[0]?.[0] ?? '';
  const last = words[words.length - 1]?.[0] ?? '';
  return (first + last).toUpperCase();
}

export function CompanyAvatar({ company, sourceFallback, size = 'sm' }: CompanyAvatarProps) {
  const label = company.trim() || (sourceFallback ?? '');
  const mono = initials(label);
  const slot = SLOTS[label.charCodeAt(0) % SLOTS.length] ?? SLOTS[0];

  return (
    <div
      aria-hidden="true"
      className={cn(
        'flex shrink-0 items-center justify-center rounded-xl font-semibold ring-1 ring-inset',
        slot,
        size === 'sm' ? 'h-8 w-8 text-[10px]' : 'h-10 w-10 text-[11px]'
      )}
    >
      {mono}
    </div>
  );
}
