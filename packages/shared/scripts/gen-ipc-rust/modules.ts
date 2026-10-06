import {
  AiGenerateRequestSchema,
  AiGenerationSaveSchema,
  AiGenerationUpdateSchema,
  AiStreamChunkSchema,
  ApplicationTrackSchema,
  ApplicationUpdateSchema,
  AutopilotCreateSchema,
  AutopilotUpdateSchema,
  DedupMarkNotDuplicateRequestSchema,
  DiscoverySearchRequestSchema,
  DiscoveryStarRequestSchema,
  DocumentImportRequestSchema,
  EmbedRequestSchema,
  HelpSearchRequestSchema,
  JobEventSchema,
  MatchResumeRequestSchema,
  MatchTextRequestSchema,
  PostingsHybridSearchRequestSchema,
  ReferralUpsertSchema,
  ResumeExtractTextSchema,
  ResumePipelineRegenerateSectionSchema,
  ResumePipelineResolveFabricationSchema,
  ResumePipelineRunSchema,
  ResumeTrimSuggestionsRequestSchema,
  ResumeValidateContentSchema,
  ScrapeBoardsRequestSchema,
  ScrapeUrlRequestSchema,
} from '../../src/schemas/index.js';
import type { ModuleSpec } from './structs.js';

export const MODULES: ModuleSpec[] = [
  {
    outFile: 'apps/desktop/src-tauri/src/ipc_contracts/scrape.rs',
    structs: [
      { rustName: 'ScrapeBoardsRequest', schema: ScrapeBoardsRequestSchema },
      { rustName: 'ScrapeUrlRequest', schema: ScrapeUrlRequestSchema },
      { rustName: 'PostingsHybridSearchRequest', schema: PostingsHybridSearchRequestSchema },
    ],
  },
  {
    outFile: 'apps/desktop/src-tauri/src/ipc_contracts/ai.rs',
    structs: [
      { rustName: 'AiGenerateRequest', schema: AiGenerateRequestSchema },
      { rustName: 'AiEmbedRequest', schema: EmbedRequestSchema },
      { rustName: 'AiGenerationSaveRequest', schema: AiGenerationSaveSchema },
      { rustName: 'AiGenerationUpdateRequest', schema: AiGenerationUpdateSchema },
    ],
  },
  {
    outFile: 'apps/desktop/src-tauri/src/ipc_contracts/documents.rs',
    structs: [
      {
        rustName: 'DocumentsImportRequest',
        schema: DocumentImportRequestSchema,
        fieldOverrides: { bytes: 'Vec<u8>' },
      },
    ],
  },
  {
    outFile: 'apps/desktop/src-tauri/src/ipc_contracts/resume.rs',
    structs: [
      {
        rustName: 'ResumeExtractTextRequest',
        schema: ResumeExtractTextSchema,
        fieldOverrides: { bytes: 'Vec<u8>' },
      },
      { rustName: 'ResumeValidateContentRequest', schema: ResumeValidateContentSchema },
    ],
  },
  {
    outFile: 'apps/desktop/src-tauri/src/ipc_contracts/resume_pipeline.rs',
    structs: [
      { rustName: 'ResumePipelineRunRequest', schema: ResumePipelineRunSchema },
      {
        rustName: 'ResumePipelineRegenerateSectionRequest',
        schema: ResumePipelineRegenerateSectionSchema,
      },
      {
        rustName: 'ResumePipelineResolveFabricationRequest',
        schema: ResumePipelineResolveFabricationSchema,
      },
    ],
  },
  {
    outFile: 'apps/desktop/src-tauri/src/ipc_contracts/autopilot.rs',
    structs: [
      { rustName: 'AutopilotCreateRequest', schema: AutopilotCreateSchema },
      { rustName: 'AutopilotUpdateRequest', schema: AutopilotUpdateSchema },
    ],
  },
  {
    outFile: 'apps/desktop/src-tauri/src/ipc_contracts/applications.rs',
    structs: [
      { rustName: 'ApplicationTrackRequest', schema: ApplicationTrackSchema },
      { rustName: 'ApplicationUpdateRequest', schema: ApplicationUpdateSchema },
    ],
  },
  {
    outFile: 'apps/desktop/src-tauri/src/ipc_contracts/matching.rs',
    structs: [
      { rustName: 'MatchResumeRequest', schema: MatchResumeRequestSchema },
      { rustName: 'MatchTextRequest', schema: MatchTextRequestSchema },
      {
        rustName: 'ResumeTrimSuggestionsRequest',
        schema: ResumeTrimSuggestionsRequestSchema,
      },
    ],
  },
  {
    outFile: 'apps/desktop/src-tauri/src/ipc_contracts/dedup.rs',
    structs: [
      { rustName: 'DedupMarkNotDuplicateRequest', schema: DedupMarkNotDuplicateRequestSchema },
    ],
  },
  {
    outFile: 'apps/desktop/src-tauri/src/ipc_contracts/discovery.rs',
    structs: [
      { rustName: 'DiscoverySearchRequest', schema: DiscoverySearchRequestSchema },
      { rustName: 'DiscoveryStarRequest', schema: DiscoveryStarRequestSchema },
    ],
  },
  {
    outFile: 'apps/desktop/src-tauri/src/ipc_contracts/referrals.rs',
    structs: [{ rustName: 'ReferralUpsertRequest', schema: ReferralUpsertSchema }],
  },
  {
    outFile: 'apps/desktop/src-tauri/src/ipc_contracts/help.rs',
    structs: [{ rustName: 'HelpSearchRequest', schema: HelpSearchRequestSchema }],
  },
  {
    outFile: 'apps/desktop/src-tauri/src/ipc_contracts/event_payloads.rs',
    structs: [
      { rustName: 'AiStreamChunk', schema: AiStreamChunkSchema },
      { rustName: 'JobEvent', schema: JobEventSchema },
    ],
  },
];
