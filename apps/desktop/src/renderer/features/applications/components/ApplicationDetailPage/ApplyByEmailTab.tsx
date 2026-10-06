import { Sparkles } from 'lucide-react';
import { useEffect, useRef, useState } from 'react';

import { type AiGenerationRecord, type Application, detectLanguage } from '@ajh/shared';
import { useTranslation } from '@ajh/translations';
import { Button, CardSkeleton, RowSkeleton } from '@ajh/ui';

import { useCanUseAI, useSelectedModel } from '@/components/ui/ModelSelector';
import { useDefaultResumeId } from '@/hooks/useDefaultResumeId';
import { generateApplicationEmail, type GenerationMeta } from '@/lib/generate';
import { COPY_FEEDBACK_MS } from '@/lib/timings';
import {
  useContactProfile,
  useDocuments,
  useDocumentText,
  useResolveJobUrl,
  useUpdateApplication,
} from '@/services';
import { useSaveAiGeneration } from '@/services/use-ai-generations';

import { extractRecipient } from '../../lib/extract-recipient';
import { useSyncedBuffer } from '../../lib/use-synced-buffer';
import { splitEmail } from './ApplyByEmailTab/email-draft';
import { EmailDraftView } from './ApplyByEmailTab/EmailDraftView';
import { EmailRecipientFields } from './ApplyByEmailTab/EmailRecipientFields';
import { EmailStatusMessages } from './ApplyByEmailTab/EmailStatusMessages';
import { useCopyFlag } from './ApplyByEmailTab/useCopyFlag';
import { useEmailRewrite } from './ApplyByEmailTab/useEmailRewrite';

interface Props {
  application: Application;
  matchingGenerations: AiGenerationRecord[];
}

/**
 * "Apply by email" tab — generates a short application email to send directly
 * to an employer contact. Recipient fields are prefilled from the job description
 * (heuristic extractor, user-editable) and persisted to the Application.
 * Generation streams through the shared AI pipeline; the user sends from their
 * own mail client via the mailto button. The settled draft (and every accepted
 * rewrite) is persisted onto the per-job aiGenerations aggregate, so it survives
 * a tab switch and is rehydrated on the next mount.
 */
export function ApplyByEmailTab({ application, matchingGenerations }: Props) {
  const { t } = useTranslation();
  const model = useSelectedModel();
  const { canUse } = useCanUseAI();

  const { isLoading: docsLoading } = useDocuments();
  const defaultResumeId = useDefaultResumeId();
  const resumeQuery = useDocumentText(defaultResumeId);
  const updateApplication = useUpdateApplication();
  const saveGeneration = useSaveAiGeneration();
  const profile = useContactProfile();

  const saved = matchingGenerations[0];
  // application.jobDescription is the primary source; the saved generation's jobAd
  // is a fallback for older records; URL resolution only fires when neither has
  // content yet (mirrors InterviewPrepTab so email generation is self-sourcing).
  const initialDesc = (application.jobDescription ?? '').trim() || (saved?.jobAd ?? '').trim();
  const resolved = useResolveJobUrl(application.jobUrl, !initialDesc);
  const jobDesc = initialDesc || (resolved.data?.description ?? '').trim();
  const resume = (resumeQuery.data ?? '').trim() || (saved?.resumeText ?? '').trim();

  // Fallback target language when there is no saved generation to copy it from:
  // detect from the job description, defaulting unknown/too-short text to English.
  const detectedLanguage = detectLanguage(jobDesc);
  const fallbackLanguage = detectedLanguage === 'unknown' ? 'en' : detectedLanguage;

  const meta: GenerationMeta = saved
    ? {
        candidateName: saved.candidateName,
        jobTitle: saved.jobTitle,
        companyName: saved.companyName,
        targetLanguage: saved.targetLanguage,
        resumeLanguage: saved.resumeLanguage,
        jobAdLanguage: saved.jobAdLanguage,
        mismatch: saved.mismatch,
        topRequirements: saved.topRequirements,
      }
    : {
        candidateName: profile.data?.fullName?.trim() ?? '',
        jobTitle: application.title,
        companyName: application.company,
        targetLanguage: fallbackLanguage,
        resumeLanguage: 'en',
        jobAdLanguage: 'en',
        mismatch: false,
        topRequirements: [],
      };

  // The email recipient IS the application's primary contact — one canonical
  // pair (`contactName`/`contactEmail`), edited here and on the Overview tab.
  // The deprecated `recipientName`/`recipientEmail` aliases are no longer read
  // or written by this surface.
  // `useSyncedBuffer`, not seeded-once `useState`: the Overview contact card
  // edits the SAME canonical columns, so a write landing there must re-seed here
  // too — otherwise whichever surface mounted first persists its stale value
  // back over the other's save (the wipe this pair was unified to prevent).
  const [recipientName, setRecipientName] = useSyncedBuffer(application.contactName);
  const [recipientEmail, setRecipientEmail] = useSyncedBuffer(application.contactEmail);
  // Both fields are server-validated (control chars / length / email shape) and a
  // rejection comes back as `{ error }` rather than a throw, so BOTH need the
  // same surfacing — silently keeping rejected text on screen would leave the
  // mailto button pointed at the stale stored address.
  const [nameError, setNameError] = useState(false);
  const [emailError, setEmailError] = useState<string | null>(null);

  // Prefill from job description when both fields are empty. The guard
  // (`recipientName || recipientEmail`) prevents re-applying once filled.
  // Including jobDesc in deps also handles the case where it loads after mount.
  useEffect(() => {
    if (recipientName || recipientEmail) return;
    if (!jobDesc) return;
    const extracted = extractRecipient(jobDesc);
    if (extracted.name) setRecipientName(extracted.name);
    if (extracted.email) setRecipientEmail(extracted.email);
    // The setters are `useState` setters returned through `useSyncedBuffer`, so
    // they are referentially stable; listed only because the lint rule cannot
    // see through the custom hook.
  }, [jobDesc, recipientEmail, recipientName, setRecipientEmail, setRecipientName]);

  // Generation state
  const [streamText, setStreamText] = useState('');
  const [isGenerating, setIsGenerating] = useState(false);
  const [genError, setGenError] = useState<string | null>(null);
  // The draft is on screen but did NOT reach the store, so it will not survive a
  // tab switch — say so instead of implying it was saved.
  const [saveFailed, setSaveFailed] = useState(false);
  const abortRef = useRef<AbortController | null>(null);
  // Mutable draft populated once generation completes, so a select-to-rewrite
  // result can be spliced back in. `null` while streaming / before the first
  // generation of this mount — the live stream or the saved record is used then
  // (see the `draft` derivation below).
  const [email, setEmail] = useState<{ subject: string; body: string } | null>(null);

  // Abort any in-flight stream on unmount to prevent quota burn on tab change.
  useEffect(() => () => abortRef.current?.abort(), []);

  // Three sources, in precedence order:
  //  1. `email` — the editable draft, set once a generation settles (and after
  //     every accepted rewrite), so edits win over everything below.
  //  2. the live stream — while a generation is running this mount, so the
  //     tokens (and, before the first token, an empty draft → skeleton) show
  //     instead of the stale saved email.
  //  3. the saved aggregate — hydrates the tab on mount, which is what makes the
  //     draft survive a tab switch / restart.
  const live = splitEmail(streamText);
  const generatingThisMount = isGenerating || streamText.length > 0;
  const draft =
    email ??
    (generatingThisMount
      ? live
      : { subject: saved?.emailSubject ?? '', body: saved?.emailBody ?? '' });
  const { subject, body } = draft;
  // A draft exists when either field has content — the saved-hydration case has
  // no `streamText`, so this can no longer key off the stream.
  const hasDraft = subject.trim().length > 0 || body.trim().length > 0;
  const canGenerate = canUse && !!model && !!resume && !!jobDesc && !isGenerating;
  const canRewrite = canUse && !!model;

  /**
   * Persist the draft onto the per-job aiGenerations aggregate (a merge-upsert
   * keyed on the normalized `jobUrl`, so it lands on the SAME row the tailor
   * flow / interview questions write to). Fire-and-forget: the draft is already
   * on screen, and the mutation invalidates the generations query on success so
   * a remount rehydrates from the store.
   *
   * Side effect (inherited from `ai_generations_save`, ADR-0001): the save also
   * upserts the parent Application with a Generate origin, which advances a
   * still-`saved` Application to `applied`. It never demotes a later stage.
   *
   * This surface owns ONLY the two email fields. Every other field is echoed
   * from the saved record, and a blank makes the backend `pick` merge keep the
   * stored value. Nothing is sourced from `meta` here: its fallbacks (the
   * contact profile's name, the Application's title/company, `en`/`en`, no
   * mismatch, `ats`) are placeholders rather than measurements, so sending them
   * would clobber real stored values whenever `saved` is undefined while a row
   * exists — a detach-then-re-track leaves exactly that orphaned FK. The
   * language pair would additionally make the merge treat this as a
   * language-bearing save and clear a genuine mismatch verdict.
   */
  const persistDraft = (next: { subject: string; body: string }) => {
    // A URL-less Application cannot be saved onto. `ai_generations_save` keys
    // BOTH the parent-Application upsert and the generation merge on the job
    // url, and both treat '' as "no match" — so each save would mint a fresh
    // `applied` Application (sorted to the top of the list) plus a generation
    // row whose FK points at that phantom, which `matchingGenerations` (an FK
    // join) can never surface. The draft would still be lost on a tab switch,
    // now with duplicate applications as a parting gift. Staying session-only
    // for these is strictly better until the save can be keyed on the id.
    if (!application.jobUrl.trim()) return;
    if (!next.subject.trim() && !next.body.trim()) return;
    setSaveFailed(false);
    saveGeneration.mutate(
      {
        candidateName: saved?.candidateName ?? '',
        jobTitle: saved?.jobTitle ?? '',
        companyName: saved?.companyName ?? '',
        resumeLanguage: saved?.resumeLanguage ?? '',
        jobAdLanguage: saved?.jobAdLanguage ?? '',
        targetLanguage: saved?.targetLanguage ?? '',
        mismatch: saved?.mismatch ?? false,
        topRequirements: saved?.topRequirements ?? [],
        mode: saved?.mode ?? '',
        // Blank so the merge keeps whatever the tailor flow stored — this
        // surface owns only the email fields.
        resumeText: '',
        coverLetterText: '',
        jobAd: jobDesc,
        jobUrl: application.jobUrl,
        board: application.board ?? '',
        emailSubject: next.subject,
        emailBody: next.body,
      },
      {
        // The command reports failure IN-BAND (`{ error }`) rather than
        // rejecting, so onError never fires for a store failure — mirror
        // `persistEmail` below and inspect the payload, or the UI would show a
        // draft it silently failed to persist.
        onSuccess: (data) => setSaveFailed('error' in data),
        onError: () => setSaveFailed(true),
      }
    );
  };

  const handleGenerate = async () => {
    if (!canGenerate) return;
    abortRef.current?.abort();
    abortRef.current = new AbortController();
    setIsGenerating(true);
    setStreamText('');
    setEmail(null);
    setGenError(null);
    setSaveFailed(false);
    try {
      const full = await generateApplicationEmail({
        resume,
        jobAd: jobDesc,
        meta,
        model,
        recipientName: recipientName.trim() || undefined,
        recipientEmail: recipientEmail.trim() || undefined,
        companyBrief: saved?.companyBrief ?? '',
        signal: abortRef.current.signal,
        onToken: (tok) => setStreamText((prev) => prev + tok),
      });
      // Freeze the final draft into editable state so rewrites can splice into it.
      const settled = splitEmail(full);
      setEmail(settled);
      persistDraft(settled);
    } catch (err) {
      if (err instanceof Error && err.name === 'AbortError') return;
      // Drop the truncated stream. Leaving it non-empty pins `generatingThisMount`
      // true for the rest of the mount, so the draft derivation never falls back
      // to the saved record: the persisted email would stay masked behind a
      // half-written fragment — with Copy and mailto acting on that fragment —
      // until the tab was remounted.
      setStreamText('');
      setGenError(t('applications.detail.email.genError'));
    } finally {
      setIsGenerating(false);
    }
  };

  // Body-only copy — the subject has its own copy button, so re-prepending
  // "Subject: …" would force the user to strip it back out before pasting into a
  // mail client's body field. The subject copy writes JUST the subject (no
  // "Subject:" prefix, no body) and has its own flag so the two never collide.
  const [copied, copyBody] = useCopyFlag(2000);
  const [subjectCopied, copySubject] = useCopyFlag(COPY_FEEDBACK_MS);

  // Select-to-rewrite: the accepted splice is committed to the editable draft and
  // persisted so the edit survives a tab switch. Splicing into `draft` (not
  // `email`) also covers a rewrite of a draft hydrated from the store and never
  // re-generated, where `email` is still null.
  const rewrite = useEmailRewrite(draft, (field, spliced) => {
    const next = { ...draft, [field]: spliced };
    setEmail(next);
    persistDraft(next);
  });

  const mailtoHref =
    recipientEmail.trim() && subject && body
      ? `mailto:${encodeURIComponent(recipientEmail.trim())}?subject=${encodeURIComponent(subject)}&body=${encodeURIComponent(body)}`
      : undefined;

  const persistName = () => {
    const val = recipientName.trim();
    if (val === application.contactName) return;
    updateApplication.mutate(
      { id: application.id, contactName: val },
      // TWO failure shapes: a validation rejection comes back in-band as
      // `{ error }`, while a transport/IPC failure rejects. Both leave the field
      // showing text that was never stored, so both must surface.
      {
        onSuccess: (data) => setNameError(!!data.error),
        onError: () => setNameError(true),
      }
    );
  };

  const persistEmail = (value: string) => {
    const val = value.trim();
    if (val === application.contactEmail) return;
    updateApplication.mutate(
      { id: application.id, contactEmail: val },
      {
        onSuccess: (data) => {
          setEmailError(data.error ? t('applications.detail.email.emailInvalid') : null);
        },
        onError: () => setEmailError(t('applications.detail.email.emailInvalid')),
      }
    );
  };

  if (
    docsLoading ||
    (!!defaultResumeId && resumeQuery.isLoading) ||
    (!initialDesc && resolved.isFetching)
  ) {
    return (
      <div className="h-full overflow-y-auto px-6 py-5">
        <CardSkeleton />
      </div>
    );
  }

  return (
    <div className="@container flex h-full min-h-0 flex-col">
      {/* Toolbar: recipient inputs + generate button */}
      <div className="shrink-0 space-y-3 border-b border-[var(--border-soft)] px-6 py-4">
        <EmailRecipientFields
          name={recipientName}
          email={recipientEmail}
          nameError={nameError}
          emailError={emailError}
          onNameChange={(v) => {
            setRecipientName(v);
            setNameError(false);
          }}
          onEmailChange={(v) => {
            setRecipientEmail(v);
            setEmailError(null);
          }}
          onNameBlur={persistName}
          onEmailBlur={persistEmail}
        />

        <Button
          variant="primary"
          disabled={!canGenerate}
          loading={isGenerating}
          onClick={() => void handleGenerate()}
          className="gap-1.5"
        >
          <Sparkles size={13} />
          {isGenerating
            ? t('applications.detail.email.generating')
            : hasDraft
              ? t('applications.detail.email.regenerate')
              : t('applications.detail.email.generate')}
        </Button>
      </div>

      {/* Preview area — aria-live so screen readers hear when generation finishes */}
      <div
        className="min-h-0 flex-1 overflow-y-auto px-6 py-4"
        aria-live="polite"
        aria-atomic="false"
      >
        <EmailStatusMessages
          canUse={canUse}
          hasResume={!!resume}
          hasJobDesc={!!jobDesc}
          hasDraft={hasDraft}
          isGenerating={isGenerating}
          genError={genError}
          saveFailed={saveFailed}
        />

        {/* Skeleton during the first tokens — avoids a bare-cursor flash */}
        {isGenerating && !streamText && (
          <div className="space-y-3">
            <RowSkeleton />
            <RowSkeleton />
          </div>
        )}

        {(hasDraft || (isGenerating && !!streamText)) && (
          <EmailDraftView
            subject={subject}
            body={body}
            isGenerating={isGenerating}
            hasDraft={hasDraft}
            canRewrite={canRewrite}
            model={model}
            locale={meta.targetLanguage}
            rewrite={rewrite}
            copied={copied}
            subjectCopied={subjectCopied}
            onCopy={() => copyBody(body)}
            onCopySubject={() => copySubject(subject)}
            mailtoHref={mailtoHref}
          />
        )}
      </div>
    </div>
  );
}
