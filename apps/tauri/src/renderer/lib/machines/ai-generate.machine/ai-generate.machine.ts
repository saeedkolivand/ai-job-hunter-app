import { createMachine } from '@/lib/machine';

/**
 * AI Document Generation state machine.
 *
 * States:
 *   idle         → waiting for user to fill in resume + job ad
 *   configuring  → user has submitted the form; validating inputs
 *   extracting   → extracting metadata (name, role, language) via LLM
 *   generating   → streaming resume + cover letter tokens
 *   done         → generation complete; user can copy/export
 *   error        → any step failed; user can retry
 *
 * Valid transitions (happy path):
 *   idle → configuring → extracting → generating → done
 *
 * Error recovery:
 *   any busy state → error → idle (via RESET)
 */

export type AiGenerateState =
  | 'idle'
  | 'configuring'
  | 'extracting'
  | 'generating'
  | 'done'
  | 'error';

export type AiGenerateEvent =
  | 'SUBMIT' // user submits the form
  | 'EXTRACTION_DONE' // metadata extraction succeeded
  | 'STREAM_DONE' // generation stream completed
  | 'ERROR' // any step failed
  | 'RESET'; // return to idle

export const aiGenerateMachine = createMachine<AiGenerateState, AiGenerateEvent>({
  transitions: {
    idle: { SUBMIT: 'configuring' },
    configuring: { EXTRACTION_DONE: 'extracting', ERROR: 'error' },
    extracting: { STREAM_DONE: 'generating', ERROR: 'error' },
    generating: { STREAM_DONE: 'done', ERROR: 'error' },
    done: { RESET: 'idle' },
    error: { RESET: 'idle' },
  },
  busyStates: ['configuring', 'extracting', 'generating'],
  errorStates: ['error'],
});
