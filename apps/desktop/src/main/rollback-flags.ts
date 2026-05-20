/**
 * Rollback flags — single source of truth for every env-var escape hatch.
 *
 * Each flag maps to one architectural phase. Setting the flag reverts that
 * phase to its previous behaviour without touching any other code path.
 *
 * ┌─────────────────────────────┬──────────────────────────────────────────┐
 * │ Env var                     │ Effect                                   │
 * ├─────────────────────────────┼──────────────────────────────────────────┤
 * │ AJH_LOW_END_MODE=1          │ Phase 1 — skip GPU acceleration switches, │
 * │                             │ set concurrency to 1, disable vibrancy.  │
 * ├─────────────────────────────┼──────────────────────────────────────────┤
 * │ AJH_EAGER_BOOT=1            │ Phase 4 — start AI and data runtimes     │
 * │                             │ immediately at bootstrap instead of      │
 * │                             │ lazily on first feature use.             │
 * ├─────────────────────────────┼──────────────────────────────────────────┤
 * │ AJH_SCRAPER_MODE=in-process │ Phase 3 — force in-process scraper       │
 * │                             │ regardless of the current default.       │
 * │   =utility-process          │ Force UtilityProcess runtime (future).   │
 * │   =http-sidecar             │ Force HTTP sidecar runtime (future).     │
 * └─────────────────────────────┴──────────────────────────────────────────┘
 *
 * All flags are read once at module load. Changing them at runtime has no
 * effect — restart the app with the new env var.
 */

export type ScraperMode = 'in-process' | 'utility-process' | 'http-sidecar';

const VALID_SCRAPER_MODES: readonly ScraperMode[] = [
  'in-process',
  'utility-process',
  'http-sidecar',
];

function parseScraperMode(raw: string | undefined): ScraperMode {
  if (raw === undefined) return 'in-process';
  if ((VALID_SCRAPER_MODES as readonly string[]).includes(raw)) return raw as ScraperMode;
  console.warn(
    `[rollback] Unknown AJH_SCRAPER_MODE "${raw}" — falling back to "in-process". ` +
      `Valid values: ${VALID_SCRAPER_MODES.join(', ')}`
  );
  return 'in-process';
}

export const rollbackFlags = {
  /**
   * Phase 1 rollback: force low-memory hardware profile.
   *
   * When set, the app skips GPU-acceleration command-line switches, limits
   * JobQueue concurrency to 1, and disables vibrancy on all windows.
   * Equivalent to setting performanceMode='low-memory' in settings, but
   * takes effect before the renderer loads so it covers startup overhead too.
   */
  lowEndMode: process.env.AJH_LOW_END_MODE === '1',

  /**
   * Phase 4 rollback: start all runtimes eagerly at bootstrap.
   *
   * When set, data and AI runtimes are both started during bootstrap() instead
   * of lazily on first use. Useful when diagnosing startup failures or when
   * testing a migration that assumes both runtimes are ready immediately.
   */
  eagerBoot: process.env.AJH_EAGER_BOOT === '1',

  /**
   * Phase 3 rollback: override the active scraper runtime implementation.
   *
   * Defaults to 'in-process' (InProcessScraperRuntime). Set to 'in-process'
   * explicitly to force the fallback even when a utility-process or sidecar
   * runtime becomes the new default in a future phase.
   */
  scraperMode: parseScraperMode(process.env.AJH_SCRAPER_MODE),
} as const;
