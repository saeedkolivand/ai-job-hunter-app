export type RadarRing = 'adopt' | 'trial' | 'assess' | 'hold';

export type RadarQuadrant =
  'renderer-ui' | 'backend-data' | 'documents-export' | 'build-ship-trust';

// What scripts/check-tech-radar.mjs should do with an entry:
//  - 'dependency': `dependencyName` (or `name` if omitted) MUST be a real
//    dependency key in a package.json / Cargo.toml on disk today.
//  - 'technique': an in-house pattern or practice — no package name exists.
//  - 'service': a hosted or locally-run service reached over HTTP, not
//    installed via a package manager (Ollama, CodeRabbit, Nominatim, …).
//  - 'not-adopted': names a REAL package/product that was deliberately never
//    added (or was removed on purpose) — exempt from the dependency check by
//    design, not because nobody wrote the check for it.
export type RadarSubjectKind = 'dependency' | 'technique' | 'service' | 'not-adopted';

export interface TechRadarEntry {
  id: string;
  name: string;
  ring: RadarRing;
  quadrant: RadarQuadrant;
  subjectKind: RadarSubjectKind;
  dependencyName?: string;
  summary: string;
  rationale: string;
  adrSlug?: string;
  lastReviewed: string;
}
