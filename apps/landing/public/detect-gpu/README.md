Vendored benchmark data from the `detect-gpu@5.0.70` npm package
(`dist/benchmarks/*.json`), self-hosted so `getGPUTier()` never calls out to
unpkg.com. See `src/engine/experience-gate.ts`.

Re-copy this directory from `node_modules/detect-gpu/dist/benchmarks/` any
time the `detect-gpu` dependency version bumps.
