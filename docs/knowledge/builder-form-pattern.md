# Resume Builder form pattern

Last updated: 2026-06-09

The Resume Builder uses **react-hook-form (RHF)** as the editing layer with a **one-way debounced sync to Zustand** as the persistence + generation boundary. This pattern avoids tight coupling between form state and the generation source.

## Architecture

```
React Component (BuilderWizard)
    ↓
useForm (RHF) → form state (live editing)
    ↓
Controller → @ajh/ui primitives (Button, Input, SelectDropdown, etc.)
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
- **FieldArrayList** (new): Wraps `useFieldArray().fields` + `remove()` for repeatable sections (replaced `RepeatableList`).
- **ContactProfileForm** (isolated): Own `useForm` instance; persist-on-blur pattern preserved; `extraLinks` via `useFieldArray`; photo kept local.
- **Schema** (`lib/schema.ts`): Zod validator for the entire wizard form; `zodResolver(builderSchema)` hydrates the form.

## Editing ↔ Persistence boundary (one-way sync)

**RHF owns the live form state; Zustand owns the persistence + generation source.** Unidirectional: RHF → Zustand, never the reverse.

**On mount:**

```typescript
// BuilderWizard.tsx
const { answers } = useResumeBuilder(); // Zustand slice
useEffect(() => {
  reset(answers); // mount-once: populate defaultValues from Zustand
}, [answers, reset]);
```

**Live editing:**

```typescript
// Debounced watcher: watch every form change and sync to Zustand
const formValues = watch();
useEffect(() => {
  const timer = setTimeout(() => {
    setResumeBuilder({ answers: formValues });
  }, 500);
  return () => clearTimeout(timer);
}, [formValues, setResumeBuilder]);
```

**Before synthesis (Build button):**

```typescript
// Flush validation before generate
const onBuild = async () => {
  const isValid = await trigger(); // force all fields to validate
  if (!formState.isValid) return; // gate on isValid

  const final = getValues(); // read final form state
  setResumeBuilder({ answers: final }); // sync one last time
  // ... invoke AI-Generate / export
};
```

## RHF patterns

- **useFieldArray** for repeatable sections (experience, education, extra links). Call `remove(index)` to delete; note the **stale-read hazard** (see lessons).
- **Controller** wraps custom @ajh/ui controls (Input, SelectDropdown, etc.) since they don't accept `ref`.
- **@ajh/ui LocationInput** for experience/education location (custom control, Controller-bound).
- **MonthYearField** — two SelectDropdown fields ("MMM", "YYYY") or "Present" checkbox; rendered as inline controls.
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
- MonthYearField (two dropdowns + Present checkbox)
- Placeholders for guidance

## Ownership

`frontend-reviewer` owns builder form work. Changes to RHF sync, schema, or step components route through `frontend-reviewer`.

See [ADR-013: Resume Builder base + handoff](decision-records/adr-013-resume-builder-base-plus-handoff.md).
