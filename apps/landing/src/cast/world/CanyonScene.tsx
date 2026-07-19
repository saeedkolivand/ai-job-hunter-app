"use client";

// The canyon scene graph (ADR-0016 scene 1 "The canyon"): the tower canyon, the
// paper storm, and the falling desk objects. Composed only -- CanyonWorld owns
// the camera, fog, lights, and the visibility gate that mounts this group only
// while the playhead is in the scene 0-2 range.

import type { QualityTier } from "@/engine/store";

import { DeskObjects } from "./DeskObjects";
import { PaperStorm } from "./PaperStorm";
import { TowerCanyon } from "./TowerCanyon";

export function CanyonScene({ tier }: { tier: QualityTier }) {
  return (
    <>
      <TowerCanyon tier={tier} />
      <PaperStorm tier={tier} />
      <DeskObjects />
    </>
  );
}
