import { type QueryClient, useMutation, useQuery, useQueryClient } from '@tanstack/react-query';

import type {
  DocumentImportRequest,
  ResumeExtractTextRequest,
  TemplateRecommendation,
  TemplateRecommendSignals,
} from '@ajh/shared';

import { useAppClient } from '@/providers/AppClientProvider';

import { keys } from '../query-client';

export const useDocuments = () => {
  const api = useAppClient();
  return useQuery({ queryKey: keys.documents.all, queryFn: () => api.documents.list() });
};

export const useDocumentText = (id: string | null | undefined) => {
  const api = useAppClient();
  return useQuery({
    queryKey: keys.documents.text(id ?? ''),
    queryFn: () => api.documents.getText(id ?? ''),
    enabled: !!id,
  });
};

/** Documents changed: the list AND the embedding status (the dashboard's "has a résumé" reads its `documents.total`). */
const invalidateDocuments = (qc: QueryClient) =>
  Promise.all([
    qc.invalidateQueries({ queryKey: keys.documents.all }),
    qc.invalidateQueries({ queryKey: keys.ai.embeddingStatus }),
  ]);

export const useImportDocument = () => {
  const api = useAppClient();
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (req: DocumentImportRequest) => api.documents.import(req),
    onSuccess: () => invalidateDocuments(qc),
  });
};

export const useRemoveDocument = () => {
  const api = useAppClient();
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (id: string) => api.documents.remove(id),
    onSuccess: () => invalidateDocuments(qc),
  });
};

export const useSetDefaultDocument = () => {
  const api = useAppClient();
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (id: string) => api.documents.setDefault(id),
    onSuccess: () => qc.invalidateQueries({ queryKey: keys.documents.all }),
  });
};

export const useExtractText = () => {
  const api = useAppClient();
  return useMutation({
    mutationFn: (req: ResumeExtractTextRequest) => api.resume.extractText(req),
  });
};

export const useRecommendTemplate = () => {
  const api = useAppClient();
  return useMutation<TemplateRecommendation, Error, TemplateRecommendSignals>({
    mutationFn: (req: TemplateRecommendSignals) => api.documents.recommendTemplate(req),
  });
};
