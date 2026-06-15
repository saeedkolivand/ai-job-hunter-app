import { useEffect, useRef, useState } from 'react';

import type { ReferralChannel } from '@ajh/shared/ipc';
import { detectLanguages } from '@ajh/shared/language-detection';

import { CONNECTION_NOTE_LIMIT, generateReferral } from '@/lib/generate';

interface Params {
  personName: string;
  personRole: string;
  companyName: string;
  jobTitle: string;
  resume: string;
  channel: ReferralChannel;
  model: string;
  canUse: boolean;
}

/**
 * Drafts a single referral message for the SELECTED channel only (one LLM call
 * per channel, never all three). Streams tokens into `draft` so the UI can show
 * them live in a {@link StreamingText}, and exposes an `abort` for the in-flight
 * call. The person's details are user-typed — there is NO LinkedIn fetch.
 */
export function useReferralDraft({
  personName,
  personRole,
  companyName,
  jobTitle,
  resume,
  channel,
  model,
  canUse,
}: Params) {
  const [draft, setDraft] = useState('');
  const [generating, setGenerating] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const abortRef = useRef<AbortController | null>(null);

  const canGenerate =
    canUse && personName.trim().length > 0 && resume.trim().length > 0 && !generating;

  const abort = () => {
    abortRef.current?.abort();
    abortRef.current = null;
    setGenerating(false);
  };

  // Clear the form's draft state after a save (the "add another" flow) — abort any
  // in-flight stream and wipe draft/error/generating back to the empty state.
  const reset = () => {
    abortRef.current?.abort();
    abortRef.current = null;
    setGenerating(false);
    setDraft('');
    setError(null);
  };

  // Abort any in-flight generation on unmount so the stream is torn down and we
  // never setState on a dead component.
  useEffect(
    () => () => {
      abortRef.current?.abort();
    },
    []
  );

  // When the channel changes, the previous channel's draft (and its ≤300
  // connection-note check) no longer applies, so abort any in-flight stream and
  // clear draft/error/generating. Skip the initial mount.
  const prevChannelRef = useRef(channel);
  useEffect(() => {
    if (prevChannelRef.current === channel) return;
    prevChannelRef.current = channel;
    abortRef.current?.abort();
    abortRef.current = null;
    setGenerating(false);
    setDraft('');
    setError(null);
  }, [channel]);

  const generate = async () => {
    if (!canGenerate) return;
    const controller = new AbortController();
    abortRef.current = controller;
    setGenerating(true);
    setError(null);
    setDraft('');
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
        // Write the message in the résumé's language — same client-side detection
        // the cover-letter/metadata path uses; `safeLocale` clamps it downstream.
        locale: detectLanguages(resume, '').resumeName,
        onToken: (tok) => setDraft((prev) => prev + tok),
        signal: controller.signal,
      });
      setDraft(text);
    } catch (err) {
      // An explicit abort is not an error to surface.
      if (!controller.signal.aborted) {
        setError(err instanceof Error ? err.message : 'Failed to draft the message');
      }
    } finally {
      if (abortRef.current === controller) abortRef.current = null;
      setGenerating(false);
    }
  };

  return { draft, generating, error, generate, abort, canGenerate, reset };
}
