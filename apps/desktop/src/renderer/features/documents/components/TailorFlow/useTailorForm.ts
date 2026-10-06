import { useEffect, useRef, useState } from 'react';
import { useForm, useWatch } from 'react-hook-form';
import { zodResolver } from '@hookform/resolvers/zod';

import { useDefaultResumeId } from '@/hooks/useDefaultResumeId';
import { shouldSeedResearchDefault } from '@/lib/research-company-default';
import { useActiveModelCapabilities } from '@/services';

import { tailorWizardSchema } from './lib/tailor-schema';
import { buildTailorDefaults, type TailorWizardState } from './lib/tailor-state';

/**
 * The wizard's RHF form: the live editing layer (`persistence.wizardForm` is a
 * one-shot seed), the capability-driven "search company" default, and the
 * résumé id the Score tab should score against.
 */
export function useTailorForm(
  persistedForm: TailorWizardState | null,
  resumeText: string | undefined,
  resumeDocId: string | undefined
) {
  // Capability-driven default for the "search company" toggle: default ON when
  // the active model can web-search. Read from the Rust capability matrix (never
  // a TS mirror), so a new provider needs no change here.
  const caps = useActiveModelCapabilities();
  const supportsWebSearch = caps.data?.supportsWebSearch ?? false;

  // Seed `defaultValues` ONCE — written back on step-advance and on generate.
  // Only a FRESH (unpersisted) form takes the capability-driven research default;
  // a restored form keeps the user's saved choice. Lazy `useState` initializer so
  // `buildTailorDefaults` runs once, not on every render.
  const startedFresh = useRef(persistedForm == null);
  const [initialForm] = useState<TailorWizardState>(
    () => persistedForm ?? buildTailorDefaults(resumeText, supportsWebSearch, resumeDocId)
  );
  const methods = useForm<TailorWizardState>({
    defaultValues: initialForm,
    resolver: zodResolver(tailorWizardSchema),
    mode: 'onChange',
  });

  // The research toggle is an RHF field; the questions/interview assistants
  // need its live value (the staged run itself takes no such field —
  // see the shared schema's doc comment on `ResumePipelineRunRequest`).
  const researchCompany = useWatch({ control: methods.control, name: 'researchCompany' });
  // The form field: which saved document backs the text about to be GENERATED.
  // It must keep matching the visible text, because `useTailorPipeline` sends
  // `resumeText: ''` whenever it is set — a mismatched id would silently generate
  // from a different résumé than the one on screen.
  const pickedResumeDocId = useWatch({ control: methods.control, name: 'resumeDocId' });

  // The Score tab asks a DIFFERENT question, and the two answers are not always
  // the same document. Its own copy says "Scored against your saved résumé, not
  // the tailored version shown here", so it wants the saved document even when
  // the editor holds something else — and the autopilot apply path seeds
  // `ap.resumeText` (see `AutopilotPage`), a snapshot that can differ from the
  // document it was taken from, which is exactly when the strict field above must
  // stay unset. Reading that field alone left this tab permanently on "Save a
  // résumé to score" for every autopilot-originated application.
  //
  // Prefer an explicitly-picked document, fall back to the default — which is what
  // the Jobs page has always scored (`useDefaultResumeId`), and what this line's
  // previous comment already claimed to do.
  const defaultResumeId = useDefaultResumeId();
  const resumeId = pickedResumeDocId ?? defaultResumeId ?? undefined;

  // Keep the "search company" default in sync with the active model's capability:
  // seed it when the capability resolves after a cold-cache seed, and RE-seed it
  // on a mid-session model switch that flips the capability — but only for a fresh
  // form and only until the user touches the toggle (RHF's dirty flag guards the
  // override, so an explicit choice is never clobbered). The DECISION is the shared
  // `shouldSeedResearchDefault` helper; RHF owns the state. `lastSeededResearch`
  // tracks the last-seeded capability (seeded from the fresh form's construction
  // value) so a no-change resolve is a no-op.
  const researchDirty = !!methods.formState.dirtyFields.researchCompany;
  const lastSeededResearch = useRef<boolean | null>(
    startedFresh.current ? (caps.isSuccess ? supportsWebSearch : null) : null
  );
  useEffect(() => {
    if (!startedFresh.current) return;
    const { seed, value } = shouldSeedResearchDefault({
      capabilityResolved: caps.isSuccess,
      supportsWebSearch,
      userTouched: researchDirty,
      lastSeededValue: lastSeededResearch.current,
    });
    if (!seed) return;
    lastSeededResearch.current = value;
    methods.setValue('researchCompany', value, { shouldDirty: false });
  }, [caps.isSuccess, supportsWebSearch, researchDirty, methods]);

  return { methods, researchCompany, resumeId };
}
