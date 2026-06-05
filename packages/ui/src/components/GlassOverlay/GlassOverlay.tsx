import { motion } from 'motion/react';

import { transition, variants } from '../../lib/motion';

interface GlassOverlayProps {
  onClick?: () => void;
  /** z-index layer — default matches --z-overlay (500) */
  zIndex?: number;
}

/**
 * Full-screen blurred backdrop used behind modals and dialogs.
 * Always rendered as an animated motion element so enter/exit is consistent.
 */
export function GlassOverlay({ onClick, zIndex = 500 }: GlassOverlayProps) {
  return (
    <motion.div
      className="fixed inset-0 bg-white/40 dark:bg-black/40 backdrop-blur-md"
      style={{ zIndex }}
      {...variants.overlay}
      transition={transition.overlay}
      onClick={onClick}
      aria-hidden="true"
    />
  );
}
