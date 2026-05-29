import { motion } from 'motion/react';

import { transition } from '@ajh/ui';

/**
 * Decorative modal backdrop — reading order bottom to top:
 * 1. heavy blur  2. contrast crush  3. bokeh blobs  4. vignette.
 * (The modal panel itself is the foreground, rendered by the wizard.)
 */
export function WizardBackdrop() {
  return (
    <>
      {/* 1 — heavy blur. backdrop-filter alone preserves contrast,
              so we follow it with a dark crush layer. */}
      <div
        className="absolute inset-0"
        style={{
          backdropFilter: 'blur(64px) saturate(120%) brightness(0.6)',
          WebkitBackdropFilter: 'blur(64px) saturate(120%) brightness(0.6)',
        }}
      />

      {/* 2 — contrast crush: semi-opaque dark tint that makes the
              blurred background unreadable without going full black. */}
      <div className="absolute inset-0 bg-background/70" />

      {/* 3 — bokeh blobs: soft ambient color on top of the dark crush. */}
      <div className="absolute inset-0 pointer-events-none overflow-hidden">
        <motion.div
          initial={{ opacity: 0 }}
          animate={{ opacity: 1 }}
          exit={{ opacity: 0 }}
          transition={transition.blob1}
          className="absolute"
          style={{
            top: '-5%',
            left: '5%',
            width: 500,
            height: 500,
            background: 'radial-gradient(circle, rgba(168,85,247,0.18) 0%, transparent 65%)',
            filter: 'blur(48px)',
          }}
        />
        <motion.div
          initial={{ opacity: 0 }}
          animate={{ opacity: 1 }}
          exit={{ opacity: 0 }}
          transition={transition.blob2}
          className="absolute"
          style={{
            bottom: '0%',
            right: '5%',
            width: 420,
            height: 420,
            background: 'radial-gradient(circle, rgba(79,70,229,0.15) 0%, transparent 65%)',
            filter: 'blur(56px)',
          }}
        />
        <motion.div
          initial={{ opacity: 0 }}
          animate={{ opacity: 1 }}
          exit={{ opacity: 0 }}
          transition={transition.blob3}
          className="absolute"
          style={{
            top: '40%',
            left: '40%',
            width: 300,
            height: 300,
            background: 'radial-gradient(circle, rgba(192,38,211,0.07) 0%, transparent 65%)',
            filter: 'blur(72px)',
          }}
        />
      </div>

      {/* 4 — vignette: darkens screen edges, naturally centres the eye. */}
      <div
        className="absolute inset-0 pointer-events-none"
        style={{
          background:
            'radial-gradient(ellipse 80% 80% at 50% 50%, transparent 35%, rgba(0,0,0,0.65) 100%)',
        }}
      />
    </>
  );
}
