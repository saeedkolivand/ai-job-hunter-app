// The 8 beats, in story order, each owning an equal slice of t-space that
// lines up with the waypoint arc in journey.ts. SceneManager mounts a beat's
// Scene when it is the active slice or an immediate neighbour (+/-1).

import type { ComponentType } from "react";

import Descent from "@/scenes/Descent";
import Features from "@/scenes/Features";
import Finale from "@/scenes/Finale";
import Fried from "@/scenes/Fried";
import Godmode from "@/scenes/Godmode";
import Hero from "@/scenes/Hero";
import Slump from "@/scenes/Slump";
import Testimonials from "@/scenes/Testimonials";

export interface Beat {
  id: string;
  range: [number, number];
  Scene: ComponentType;
}

const S = 1 / 8;

export const BEATS: Beat[] = [
  { id: "hero", range: [0 * S, 1 * S], Scene: Hero },
  { id: "slump", range: [1 * S, 2 * S], Scene: Slump },
  { id: "descent", range: [2 * S, 3 * S], Scene: Descent },
  { id: "fried", range: [3 * S, 4 * S], Scene: Fried },
  { id: "godmode", range: [4 * S, 5 * S], Scene: Godmode },
  { id: "features", range: [5 * S, 6 * S], Scene: Features },
  { id: "testimonials", range: [6 * S, 7 * S], Scene: Testimonials },
  { id: "finale", range: [7 * S, 8 * S], Scene: Finale },
];
