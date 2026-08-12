import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';

import { EVENT_CHANNELS, type PipelineStageEvent } from '@ajh/shared';
import type {
  ResumePipelineRegenerateSectionRequest,
  ResumePipelineResolveFabricationRequest,
  ResumePipelineRunRequest,
} from '@ajh/shared/schemas';

import { asyncUnsub } from '../../utils.js';

/**
 * Transport for the staged résumé pipeline. Thin by design — the run's
 * semantics (what the ids mean, why the draft stream is not the completion
 * signal) live in `ResumePipelineContract`, not here.
 */
export const resumePipeline = {
  run: (req: ResumePipelineRunRequest) => invoke('resume_pipeline_run', { req }),
  get: (runId: string) => invoke('resume_pipeline_get', { runId }),
  listForJob: (jobUrl: string) => invoke('resume_pipeline_list_for_job', { jobUrl }),
  regenerateSection: (req: ResumePipelineRegenerateSectionRequest) =>
    invoke('resume_pipeline_regenerate_section', { req }),
  resolveFabrication: (req: ResumePipelineResolveFabricationRequest) =>
    invoke('resume_pipeline_resolve_fabrication', { req }),
  onStage: (handler: (event: PipelineStageEvent) => void) =>
    asyncUnsub(() =>
      listen<PipelineStageEvent>(EVENT_CHANNELS.pipeline.stage, (e) => handler(e.payload))
    ),
};
