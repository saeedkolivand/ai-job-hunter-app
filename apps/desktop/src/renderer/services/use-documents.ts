import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';

import type { DocumentImportRequest, ResumeExtractTextRequest } from '@ajh/shared';

import { keys } from './query-client';

export const useDocuments = () =>
  useQuery({
    queryKey: keys.documents.all,
    queryFn: () => window.api.documents.list(),
  });

export const useImportDocument = () => {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (req: DocumentImportRequest) => window.api.documents.import(req),
    onSuccess: () => qc.invalidateQueries({ queryKey: keys.documents.all }),
  });
};

export const useRemoveDocument = () => {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (id: string) => window.api.documents.remove(id),
    onSuccess: () => qc.invalidateQueries({ queryKey: keys.documents.all }),
  });
};

export const useExtractText = () =>
  useMutation({
    mutationFn: (req: ResumeExtractTextRequest) => window.api.resume.extractText(req),
  });
