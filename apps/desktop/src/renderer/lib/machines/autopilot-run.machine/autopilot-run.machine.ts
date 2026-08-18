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
    // `idle` doubles as "this mount has not seen this autopilot run", which is
    // the state every card starts in after a navigation — the run state is
    // component-local while the run itself lives in the backend. So idle accepts
    // the whole mid-run vocabulary, not just the start: a page remounted during
    // a run (or a card watching a SCHEDULED run it never clicked) receives
    // `scrape_done`/`complete` with no preceding `scrape_start`, and dropping
    // those left the card claiming idle for a run that was still going.
    idle: {
      START: 'scraping',
      SCRAPE_START: 'scraping',
      SCRAPE_DONE: 'ranking',
      RANK_DONE: 'ranking',
      COMPLETE: 'done',
      CANCEL: 'cancelled',
      ERROR: 'error',
      RESET: 'idle',
    },
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
    // A terminal state is terminal for THIS run only. The next run of the same
    // autopilot — the scheduler's, or one started from another mount — announces
    // itself with `scrape_start`, and without this arm the card would sit on the
    // previous run's outcome while a new one streamed underneath it.
    done: { SCRAPE_START: 'scraping', RESET: 'idle' },
    cancelled: { SCRAPE_START: 'scraping', RESET: 'idle' },
    error: { SCRAPE_START: 'scraping', RESET: 'idle' },
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
