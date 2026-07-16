# Resume Builder form pattern

Last updated: 2026-07-16

The Resume Builder uses **react-hook-form (RHF)** as the editing layer with a **one-way debounced sync to Zustand** as the persistence + generation boundary. This pattern avoids tight coupling between form state and the generation source.

## Architecture

```
React Component (BuilderWizard)
    ↓
useForm (RHF) → form state (live editing)
    ↓
Controller → @ajh/ui primitives (Button, Input, Dropdown, etc.)
    ↓
useFieldArray (FieldArrayList) → repeatable sections
    ↓
onSubmit → flush & validate (formState.isValid gates Build)
    ↓
resumeBuilder store (Zustand) → persistence + generation source
```

## Key components

- **BuilderWizard** (`features/resume-builder/components/BuilderWizard/`): RHF form root with `useForm()` + `FormProvider`. Binds all wizard steps via `Controller`.
- **Wizard steps** (`features/resume-builder/components/wizard-steps/`): `StepContact`, `StepExperience`, `StepEducation`, `StepSkills`, `StepExtras`, `StepSummary`, `StepReview`. Each step uses `Controller` to bind `@ajh/ui` controls + `useFieldArray` for repeatable sections.
- **FieldArrayList**: Wraps `useFieldArray().fields` + `remove()` for repeatable sections (replaced `RepeatableList`).
- **ContactProfileForm** (isolated): Own `useForm` instance; persist-on-blur pattern preserved; `extraLinks` via `useFieldArray`; photo kept local.
- **Schema** (`lib/schema.ts`): Zod validator for the entire wizard form; `zodResolver(builderSchema)` hydrates the form.

## Editing ↔ Persistence boundary (one-way sync)

**RHF owns the live form state; Zustand owns the persistence + generation source.** Unidirectional: RHF → Zustand, never the reverse.

**On mount:**

```typescript
// BuilderWizard.tsx — seed form from Zustand ONE time, never reset on store changes
const formRef = useRef(useSessionStore.getState().resumeBuilder.answers);
const methods = useForm({
  defaultValues: toFormValues(formRef.current),
  resolver: zodResolver(builderSchema),
});
// Form does not reset on Zustand changes — blank-default only on remount
```

**Live editing:**

```typescript
// Debounced watcher (SYNC_DEBOUNCE_MS = 350): watch form changes and sync to Zustand
methods.watch((values) => {
  const timer = setTimeout(() => {
    // Merge form values with initial answers, sync to slice
    const merged = { ...formRef.current, ...methods.getValues() };
    setResumeBuilder({ answers: merged });
  }, SYNC_DEBOUNCE_MS);
  return () => clearTimeout(timer);
});
```

**Before synthesis (Build button):**

```typescript
// Build button disabled by buildDisabled = !isComplete || !canUseAI || !formState.isValid || isGenerating
const onValid = () => {
  const final = methods.getValues(); // read final form state
  setResumeBuilder({ answers: final }); // sync one last time
  onGenerate(); // invoke AI-Generate
};
// Button wires: methods.handleSubmit(onValid)
```

## RHF patterns

- **useFieldArray** for repeatable sections (experience, education, extra links). Call `remove(index)` to delete; note the **stale-read hazard** (see lessons).
- **Controller** wraps custom @ajh/ui controls (Input, Dropdown, etc.) since they don't accept `ref`.
- **@ajh/ui LocationInput** for experience/education location (custom control, Controller-bound).
- **MonthYearField** — two Dropdown fields ("MMM", "YYYY") with a read-only "Present" state when enabled; rendered as inline controls. The Present checkbox itself lives in StepExperience.
- **Form-level validation**: `formState.isValid` gates the Build button until all required fields pass schema validation.

## ContactProfileForm (isolated)

Separate form instance from the wizard. Persists changes on blur (not debounced). Owns:

- contact data (name, email, phone, URLs)
- photo (kept local state until export)
- extraLinks (via `useFieldArray`)

**Guarded reset on profile identity change:**

```typescript
// When the user imports a conflicting profile, reset the form
useEffect(() => {
  if (profileChanged) {
    reset(newProfile);
  }
}, [profileChanged, newProfile, reset]);
```

## UI traits

- Visible glass textareas (not hidden)
- Full-width inputs + 50/50 row layouts
- Inline validation errors
- LocationInput control (custom: city + country dropdown)
- MonthYearField (two dropdowns with read-only Present state; checkbox in StepExperience)
- Placeholders for guidance

## Ownership

`frontend-author` implements builder form work; `frontend-reviewer` (and `ui-ux-expert` for visual/UX changes) audits. Changes to RHF sync, schema, or step components route through the author/critic pair.

See [ADR-013: Resume Builder base + handoff](decision-records/adr-013-resume-builder-base-plus-handoff.md).
