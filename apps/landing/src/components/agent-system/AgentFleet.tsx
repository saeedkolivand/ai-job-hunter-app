'use client';

import { useReveal } from './hooks';
import { BeltSection } from './sections/BeltSection';
import { FleetMapSection } from './sections/FleetMapSection';
import { HeroSection } from './sections/HeroSection';
import { IntakeSection } from './sections/IntakeSection';
import { TogetherSection } from './sections/TogetherSection';
import { VersusSection } from './sections/VersusSection';

// The /agent-system page. State + DOM-measuring effects live in ./hooks.ts
// and are called from wherever their DOM lives (useReveal spans every
// section's `.reveal`/`.draw` markup, so it stays here at the root; the belt
// scrub and fleet-map link geometry are entirely local to their own section
// and are called from there instead).
export function AgentFleet() {
  const rootRef = useReveal();

  return (
    <div className="agent-fleet" ref={rootRef}>
      <HeroSection />
      <IntakeSection />
      <BeltSection />
      <FleetMapSection />
      <TogetherSection />
      <VersusSection />
    </div>
  );
}
