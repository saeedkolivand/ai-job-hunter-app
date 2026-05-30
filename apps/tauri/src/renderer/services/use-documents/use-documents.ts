import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';

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

export const useImportDocument = () => {
  const api = useAppClient();
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (req: DocumentImportRequest) => api.documents.import(req),
    onSuccess: () => qc.invalidateQueries({ queryKey: keys.documents.all }),
  });
};

export const useRemoveDocument = () => {
  const api = useAppClient();
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (id: string) => api.documents.remove(id),
    onSuccess: () => qc.invalidateQueries({ queryKey: keys.documents.all }),
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
