import { createMachine } from '@/lib/machine';

/**
 * Autopilot execution state machine.
 *
 * Tracks the lifecycle of a single autopilot run. Autopilot is a discovery
 * agent: it finds & ranks matching jobs, then saves them for review — it never
 * applies (the user applies with the tailoring assistant).
 *   idle → scraping → ranking → done
 *                              ↘ cancelled
 *   any state → error
 *
 * The step strings emitted by the Rust backend map to events:
 *   scrape_start  → SCRAPE_START
 *   scrape_done   → SCRAPE_DONE
 *   rank_done     → RANK_DONE
 *   complete      → COMPLETE
 *   cancelled     → CANCEL
 */

export type AutopilotRunState = 'idle' | 'scraping' | 'ranking' | 'done' | 'cancelled' | 'error';

export type AutopilotRunEvent =
  | 'START'
  | 'SCRAPE_START'
  | 'SCRAPE_DONE'
  | 'RANK_DONE'
  | 'COMPLETE'
  | 'CANCEL'
  | 'ERROR'
  | 'RESET';

export const autopilotRunMachine = createMachine<AutopilotRunState, AutopilotRunEvent>({
  transitions: {
    idle: { START: 'scraping', SCRAPE_START: 'scraping', RESET: 'idle' },
    scraping: {
      SCRAPE_DONE: 'ranking',
      RANK_DONE: 'ranking',
      COMPLETE: 'done',
      CANCEL: 'cancelled',
      ERROR: 'error',
    },
    ranking: {
      RANK_DONE: 'ranking',
      COMPLETE: 'done',
      CANCEL: 'cancelled',
      ERROR: 'error',
    },
    done: { RESET: 'idle' },
    cancelled: { RESET: 'idle' },
    error: { RESET: 'idle' },
  },
  busyStates: ['scraping', 'ranking'],
  errorStates: ['error'],
});

/** Map a Rust step string to an AutopilotRunEvent. */
export function stepToEvent(step: string): AutopilotRunEvent | null {
  switch (step) {
    case 'scrape_start':
      return 'SCRAPE_START';
    case 'scrape_done':
      return 'SCRAPE_DONE';
    case 'rank_done':
      return 'RANK_DONE';
    case 'complete':
      return 'COMPLETE';
    case 'cancelled':
      return 'CANCEL';
    default:
      return null;
  }
}

export const RUN_STATE_LABEL: Record<AutopilotRunState, string> = {
  idle: 'Idle',
  scraping: 'Scraping…',
  ranking: 'Ranking…',
  done: 'Done',
  cancelled: 'Cancelled',
  error: 'Error',
};
