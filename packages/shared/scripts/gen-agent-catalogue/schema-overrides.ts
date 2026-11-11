import { type FieldSources, toInputJsonSchema } from './field-sources.js';
import { type CatalogueEntry, fail } from './model.js';

/** Wrapper args whose call site the generator cannot resolve to a Zod type by name — the renderer
 *  passes a rest-destructured intersection (`autopilot_update`), an `unknown` (`job_preferences_set`)
 *  or an `Omit<…> & …` (`resume_pipeline_run`) — mapped to the schema const that IS their contract.
 *  Static, like the allowlist; `applyOverrides` fails if a row no longer matches the catalogue, so a
 *  rename cannot leave a stale row behind. */
const SCHEMA_OVERRIDES: readonly { command: string; arg: string; schema: string }[] = [
  { command: 'autopilot_update', arg: 'req', schema: 'AutopilotUpdateSchema' },
  { command: 'job_preferences_set', arg: 'prefs', schema: 'JobPreferencesSchema' },
  { command: 'resume_pipeline_run', arg: 'req', schema: 'ResumePipelineRunSchema' },
];

export function applyOverrides(entries: Map<string, CatalogueEntry>, sources: FieldSources): void {
  for (const { command, arg, schema } of SCHEMA_OVERRIDES) {
    const target = entries.get(command)?.args.find((a) => a.name === arg);
    if (!target) fail(`schema override ${command}.${arg} matches no catalogued argument`);
    const zodSchema = sources.zodSchemas[schema];
    if (!zodSchema) fail(`schema override ${command}.${arg}: \`${schema}\` is not exported`);
    target.schema = toInputJsonSchema(zodSchema);
  }
}
