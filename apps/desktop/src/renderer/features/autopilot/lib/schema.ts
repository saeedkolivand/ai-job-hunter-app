import { z } from 'zod';

import type { WizardState } from '@/features/autopilot/types';

/**
 * Zod schema for the Autopilot creation wizard form — the single source of truth
 * for the in-wizard editing layer (react-hook-form + `zodResolver`). It mirrors
 * {@link WizardState} exactly so `useForm<WizardState>` and the resolver agree.
 *
 * This is deliberately NOT the IPC `AutopilotCreate` shape: `keywords` /
 * `excludeKeywords` are editable comma strings here (split on save), and
 * `workType: 'any'` + an empty `dateFilter` are form-level sentinels that
 * {@link wizardStateToPayload} maps away. Number/enum controls are bounded by
 * their own widgets, so only `name` and `query` are user-failable — both live on
 * step 0, which is what gates the wizard's "Next".
 *
 * `boards` has no upper bound here: the picker only toggles catalog entries and
 * a normalization effect strips unknown ids (see `StepTarget`), so the board
 * catalog itself bounds the selection. The real defense against an oversized
 * payload is the server-side registry dedup+truncate in the Rust scrape engine.
 *
 * No field uses `.default()`: the wizard always seeds complete `defaultValues`
 * (`buildDefaults` / `autopilotToWizardState`), so the schema's input and output
 * types match (a zod `.default()` input/output mismatch otherwise breaks the
 * `useForm`/`zodResolver` generics). `name` / `query` messages are i18n KEYS the
 * steps resolve through `WizardField`'s `error` prop via `t(...)`.
 */
export const autopilotWizardSchema = z.object({
  name: z.string().trim().min(1, 'autopilot.wizard.validation.nameRequired'),
  boards: z.array(z.string().min(1)).min(1, 'autopilot.wizard.validation.missingFields'),
  query: z.string().trim().min(1, 'autopilot.wizard.validation.queryRequired'),
  location: z.string(),
  countryCode: z.string().optional(),
  workType: z.enum(['remote', 'hybrid', 'on-site', 'any']),
  amount: z.number().int().min(1).max(500),
  dateFilter: z.string(),
  watchedCompaniesOnly: z.boolean(),
  minMatchScore: z.number().min(0).max(100),
  keywords: z.string(),
  excludeKeywords: z.string(),
  resumeText: z.string(),
  assistant: z.boolean(),
  assistantProvider: z.string().optional(),
  assistantModel: z.string().optional(),
  assistantBaseUrl: z.string().optional(),
  schedule: z.enum(['manual', 'hourly', 'daily', 'twice_daily']),
  scheduleHour: z.number().int().min(0).max(23),
  scheduleMinute: z.number().int().min(0).max(59),
});

// Compile-time guard: the inferred schema type must stay in lockstep with the
// hand-written WizardState the store + helpers use. If either drifts, this errors.
type _SchemaMatchesState =
  z.infer<typeof autopilotWizardSchema> extends WizardState
    ? WizardState extends z.infer<typeof autopilotWizardSchema>
      ? true
      : never
    : never;
const _assert: _SchemaMatchesState = true;
void _assert;
