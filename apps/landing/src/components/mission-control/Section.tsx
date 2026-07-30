import type { ReactNode } from 'react';

// The `mc-section` + `__eyebrow` + `__title` wrapper repeated across every
// /mission-control section (Delivery, Work, Quality, Community, Maintainer
// actions). Must render byte-identical markup at every call site.
export function Section({
  label,
  eyebrow,
  title,
  children,
}: {
  label: string;
  eyebrow: string;
  title: string;
  children: ReactNode;
}) {
  return (
    <section className="mc-section" aria-label={label}>
      <p className="mc-section__eyebrow">{eyebrow}</p>
      <h2 className="mc-section__title">{title}</h2>
      {children}
    </section>
  );
}
