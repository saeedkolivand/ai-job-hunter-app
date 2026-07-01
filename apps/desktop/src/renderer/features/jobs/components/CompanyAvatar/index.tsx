import { useEffect, useState } from 'react';

import { cn, Image } from '@ajh/ui';

import { useCompanyLogo } from '@/services';
import { useFetchCompanyLogos } from '@/store/preferences-store';

interface CompanyAvatarProps {
  company: string;
  sourceFallback?: string;
  size?: 'sm' | 'md';
}

// Deterministic color slot — 6 semantic token pairs so the monogram stays
// on-theme in both light and dark without raw palette classes.
// charCodeAt(0) % 6 maps the company label to a slot.
const SLOTS = [
  'bg-brand/10 text-brand ring-brand/20', // teal (brand)
  'bg-brand-2/10 text-brand-2 ring-brand-2/20', // coral (brand-2)
  'bg-action-run/10 text-action-run ring-action-run/20', // green
  'bg-action-edit/10 text-action-edit ring-action-edit/20', // blue
  'bg-destructive/10 text-destructive ring-destructive/20', // red
  'bg-muted text-foreground/60 ring-foreground/10', // neutral
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

  // Logo enrichment: only fires when the user has opted in.
  // Zero requests when the preference is off or company is blank.
  const logosEnabled = useFetchCompanyLogos();
  const logoUrl = useCompanyLogo(company, logosEnabled);

  // Track whether the image itself failed to load (404, CORS on img-src, etc.).
  // When failed: remove the image layer entirely so the monogram is always visible.
  // Reset whenever logoUrl changes (new company) so each new URL gets a fresh attempt.
  const [logoFailed, setLogoFailed] = useState(false);
  useEffect(() => {
    setLogoFailed(false);
  }, [logoUrl]);

  // Show the logo layer when: setting on + URL resolved + image hasn't errored.
  const showLogoLayer = logosEnabled && !!logoUrl && !logoFailed;

  return (
    <div
      aria-hidden="true"
      className={cn(
        'relative flex shrink-0 items-center justify-center overflow-hidden rounded-xl font-semibold ring-1 ring-inset',
        slot,
        size === 'sm' ? 'h-8 w-8 text-[10px]' : 'h-10 w-10 text-[11px]'
      )}
    >
      {/* Monogram — always rendered; only visually hidden while the logo layer
          is active (URL present, image not yet errored). When the image errors,
          logoFailed=true collapses showLogoLayer and the monogram reappears. */}
      <span className={showLogoLayer ? 'invisible' : undefined}>{mono}</span>

      {/* Logo layer — absolutely fills the avatar over the monogram.
          preview=false disables the click-to-zoom lightbox (decorative, not interactive).
          onError restores the monogram: sets logoFailed=true which collapses
          showLogoLayer so the monogram span loses its 'invisible' class. */}
      {showLogoLayer && (
        <Image
          key={logoUrl}
          src={logoUrl}
          alt=""
          preview={false}
          className="absolute inset-0 h-full w-full object-cover"
          onError={() => setLogoFailed(true)}
        />
      )}
    </div>
  );
}
