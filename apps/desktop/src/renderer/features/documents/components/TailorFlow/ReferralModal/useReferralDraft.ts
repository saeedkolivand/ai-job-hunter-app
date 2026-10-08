import { useEffect, useRef } from 'react';

import type { ReferralChannel } from '@ajh/shared/ipc';
import { detectLanguages } from '@ajh/shared/language-detection';
import { useTranslation } from '@ajh/translations';

import { CONNECTION_NOTE_LIMIT, generateReferral, generateReferralImprove } from '@/lib/generate';
import { useReferralDraftStore } from '@/store/session-store';

interface Params {
  /** The job the draft belongs to; keys the session-held run (see `ReferralDraftSlice`). */
  jobUrl: string;
  personName: string;
  personRole: string;
  companyName: string;
  jobTitle: string;
  resume: string;
  channel: ReferralChannel;
  model: string;
  canUse: boolean;
}

/** The backend's raw transport failure (`Stream error: …`) is not user-facing; localize it. */
const STREAM_ERROR_PREFIX = 'Stream error';

// The latest started run. Module-level (not a per-mount ref) so the stream outlives
// the modal (AGENTS.md rule 16); late writes from a superseded/reset run are dropped.
let current: AbortController | null = null;

const store = () => useReferralDraftStore.getState();

/** The saved form fields for `jobUrl`, so a reopened modal can restore them. */
export function readReferralSeed(jobUrl: string) {
  const s = store().referralDraft;
  return s.jobUrl === jobUrl ? s : null;
}

/**
 * Drafts a single referral message for the SELECTED channel only (one LLM call
 * per channel, never all three). Streams tokens into `draft` so the UI can show
 * them live in a {@link StreamingText}, and exposes an `abort` for the in-flight
 * call. The person's details are user-typed — there is NO LinkedIn fetch.
 */
export function useReferralDraft({
  jobUrl,
  personName,
  personRole,
  companyName,
  jobTitle,
  resume,
  channel,
  model,
  canUse,
}: Params) {
  const { t } = useTranslation();
  const slice = useReferralDraftStore((s) => s.referralDraft);
  const mine = slice.jobUrl === jobUrl && slice.channel === channel;
  const draft = mine ? slice.draft : '';
  const generating = mine && slice.generating;
  const error = mine ? slice.error : null;
  // Writes are dropped once this run was superseded or reset.
  const write = (c: AbortController, patch: Partial<typeof slice>) => {
    if (current === c) store().setReferralDraft(patch);
  };

  const canGenerate =
    canUse && personName.trim().length > 0 && resume.trim().length > 0 && !generating;

  const abort = () => {
    current?.abort();
    if (mine) store().setReferralDraft({ generating: false });
  };

  // Clear the form's draft state after a save (the "add another" flow) — abort any
  // in-flight stream and wipe draft/error/generating back to the empty state.
  const reset = () => {
    current?.abort();
    current = null;
    store().resetReferralDraft();
  };

  // When the channel changes, the previous channel's draft (and its ≤300
  // connection-note check) no longer applies, so abort any in-flight stream and
  // clear draft/error/generating. Skip the initial mount.
  const prevChannelRef = useRef(channel);
  useEffect(() => {
    if (prevChannelRef.current === channel) return;
    prevChannelRef.current = channel;
    if (store().referralDraft.jobUrl !== jobUrl) return;
    current?.abort();
    current = null;
    store().resetReferralDraft();
  }, [channel, jobUrl]);

  const errorText = (err: unknown, fallback: string) => {
    if (!(err instanceof Error)) return fallback;
    return err.message.startsWith(STREAM_ERROR_PREFIX)
      ? t('autopilot.referral.streamError')
      : err.message;
  };

  const generate = async () => {
    if (!canGenerate) return;
    current?.abort();
    const controller = new AbortController();
    current = controller;
    store().setReferralDraft({
      jobUrl,
      personName: personName.trim(),
      personRole: personRole.trim(),
      channel,
      draft: '',
      generating: true,
      error: null,
    });
    try {
      const text = await generateReferral({
        personName: personName.trim(),
        personRole: personRole.trim() || undefined,
        companyName,
        jobTitle,
        resume,
        format: channel,
        charLimit: channel === 'connection_note' ? CONNECTION_NOTE_LIMIT : undefined,
        model,
        // Write the message in the résumé's language — pass the ISO 639-1 code
        // (not the display name) so `safeLocale` downstream doesn't collapse it to 'en'.
        locale: detectLanguages(resume, '').resume,
        onToken: (tok) => write(controller, { draft: store().referralDraft.draft + tok }),
        signal: controller.signal,
      });
      write(controller, { draft: text });
    } catch (err) {
      // An explicit abort is not an error to surface.
      if (!controller.signal.aborted) {
        write(controller, { error: errorText(err, 'Failed to draft the message') });
      }
    } finally {
      write(controller, { generating: false });
    }
  };

  /**
   * Revise the current draft per a user instruction. Streams the revised draft
   * into the same draft state (replaces in-place), reusing abort/generating/error.
   * Respects `canGenerate` gating and requires a non-empty draft to act on.
   *
   * The existing draft is preserved (not cleared) until the first streaming token
   * arrives, so a failed or aborted improve never destroys the user's draft.
   *
   * SECURITY: `instruction` must be user-originated. Never pass scraped or
   * AI-generated content as the instruction.
   */
  const improve = async (instruction: string) => {
    if (!canGenerate || !draft) return;
    // Snapshot the current draft — if the request fails or is aborted, restore it.
    const snapshot = draft;
    current?.abort();
    const controller = new AbortController();
    current = controller;
    store().setReferralDraft({ generating: true, error: null });
    // Do NOT clear the draft up front. The first streaming token replaces it.
    let firstToken = true;
    try {
      const text = await generateReferralImprove({
        personName: personName.trim(),
        personRole: personRole.trim() || undefined,
        companyName,
        jobTitle,
        resume,
        draft: snapshot,
        instruction,
        format: channel,
        charLimit: channel === 'connection_note' ? CONNECTION_NOTE_LIMIT : undefined,
        model,
        locale: detectLanguages(resume, '').resume,
        onToken: (tok) => {
          if (firstToken) {
            // Replace the snapshot with the first streaming token.
            firstToken = false;
            write(controller, { draft: tok });
          } else {
            write(controller, { draft: store().referralDraft.draft + tok });
          }
        },
        signal: controller.signal,
      });
      write(controller, { draft: text });
    } catch (err) {
      if (!controller.signal.aborted) {
        write(controller, { error: errorText(err, 'Failed to improve the draft') });
      }
      // Restore the snapshot so the draft survives a failed or aborted improve.
      write(controller, { draft: snapshot });
    } finally {
      write(controller, { generating: false });
    }
  };

  return { draft, generating, error, generate, improve, abort, canGenerate, reset };
}
