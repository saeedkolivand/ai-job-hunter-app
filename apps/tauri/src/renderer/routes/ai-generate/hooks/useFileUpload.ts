import { ACCEPTED_EXTS, MAX_BYTES } from '../constants';

export function useFileUpload(
  setUploadError: (error: string | null) => void,
  setUploading: (uploading: 'resume' | 'jobAd' | null) => void,
  setResume: (text: string) => void,
  setJobAd: (text: string) => void,
  extractTextMutation: {
    mutateAsync: (args: { name: string; bytes: Uint8Array }) => Promise<unknown>;
  },
  t: (key: string, params?: Record<string, unknown>) => string
) {
  const handleUpload = async (target: 'resume' | 'jobAd', file: File) => {
    setUploadError(null);
    const ext = file.name.toLowerCase().split('.').pop() ?? '';
    if (!ACCEPTED_EXTS.includes(ext as (typeof ACCEPTED_EXTS)[number])) {
      setUploadError(t('aiGenerate.errors.unsupportedFileType', { ext }));
      return;
    }
    if (file.size > MAX_BYTES) {
      setUploadError(t('aiGenerate.errors.fileTooLarge'));
      return;
    }
    setUploading(target);
    try {
      const bytes = new Uint8Array(await file.arrayBuffer());
      const res = (await extractTextMutation.mutateAsync({ name: file.name, bytes })) as {
        text: string;
      };
      const text = (res?.text ?? '').trim();
      if (!text) {
        setUploadError(t('aiGenerate.errors.couldNotExtract'));
        return;
      }
      if (target === 'resume') setResume(text);
      else setJobAd(text);
    } catch (err) {
      setUploadError(err instanceof Error ? err.message : t('aiGenerate.errors.uploadFailed'));
    } finally {
      setUploading(null);
    }
  };

  return { handleUpload };
}
