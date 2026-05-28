import { useEffect, useRef, useState } from 'react';

import type { AiStreamChunk, JobEvent } from '@ajh/shared';

import i18n from '@/i18n';
import { useTranslation } from '@/lib/i18n';
import {
  useAIStream,
  useExtractText,
  useGenerateAI,
  useGetOrCreateConversation,
  useJobEvents,
} from '@/services';

const ACCEPTED_EXTS = ['pdf', 'docx', 'txt', 'md', 'markdown'] as const;
const MAX_BYTES = 25 * 1024 * 1024;

export interface Msg {
  id: string;
  role: 'user' | 'assistant';
  content: string;
  displayedContent?: string;
  jobId?: string;
  isStreaming?: boolean;
}

export function useChat() {
  const { t } = useTranslation();
  const [messages, setMessages] = useState<Msg[]>([]);
  const [input, setInput] = useState('');
  const [streaming, setStreaming] = useState(false);
  const [isInputFocused, setIsInputFocused] = useState(false);
  const [resumeText, setResumeText] = useState<string>('');
  const [resumeFileName, setResumeFileName] = useState<string>('');
  const [uploading, setUploading] = useState(false);
  const [copiedMessageId, setCopiedMessageId] = useState<string | null>(null);
  const activeJobRef = useRef<string | null>(null);
  const scrollRef = useRef<HTMLDivElement>(null);
  const fileInputRef = useRef<HTMLInputElement>(null);
  const selectedModel = useRef('');
  const generateConfig = useRef({ provider: 'ollama', baseUrl: '' as string | undefined });

  const getOrCreateConversation = useGetOrCreateConversation();
  const extractText = useExtractText();
  const generateAI = useGenerateAI();

  // Load conversation once on mount
  const getOrCreateRef = useRef(getOrCreateConversation.mutateAsync);
  getOrCreateRef.current = getOrCreateConversation.mutateAsync;
  useEffect(() => {
    void getOrCreateRef.current().catch((error: unknown) => {
      console.error('Failed to load messages:', error);
    });
  }, []);

  // Subscribe to streaming deltas
  useAIStream((chunk: AiStreamChunk) => {
    const raw = chunk;
    if (raw.jobId !== activeJobRef.current) return;
    if (raw.error) {
      activeJobRef.current = null;
      setStreaming(false);
      const errorMessage = raw.error.message;
      setMessages((prev) => [
        ...prev,
        {
          id: crypto.randomUUID(),
          role: 'assistant' as const,
          content: errorMessage,
        },
      ]);
      return;
    }
    setMessages((prev) => {
      const last = prev[prev.length - 1];
      if (!last || last.jobId !== raw.jobId) {
        return [
          ...prev,
          {
            id: crypto.randomUUID(),
            role: 'assistant',
            content: raw.delta,
            displayedContent: raw.delta,
            jobId: raw.jobId,
          },
        ];
      }
      const updated: Msg = {
        ...last,
        content: last.content + raw.delta,
        displayedContent: last.content + raw.delta,
      };
      return [...prev.slice(0, -1), updated];
    });
    if (raw.done) {
      activeJobRef.current = null;
      setStreaming(false);
    }
  });

  // Subscribe to job events to reset streaming state on completion/failure
  useJobEvents((event: JobEvent) => {
    const evt = event as { type: string; jobId: string };
    if (evt.jobId !== activeJobRef.current) return;
    if (evt.type === 'job.completed' || evt.type === 'job.failed' || evt.type === 'job.cancelled') {
      activeJobRef.current = null;
      setStreaming(false);
    }
  });

  // Handle resume upload
  const handleResumeUpload = async (file: File) => {
    const ext = file.name.toLowerCase().split('.').pop() ?? '';
    if (!ACCEPTED_EXTS.includes(ext as (typeof ACCEPTED_EXTS)[number])) {
      alert(t('ai.errors.unsupportedFileType', { ext }));
      return;
    }
    if (file.size > MAX_BYTES) {
      alert(t('ai.errors.fileTooLarge'));
      return;
    }
    setUploading(true);
    try {
      const bytes = new Uint8Array(await file.arrayBuffer());
      const res = (await extractText.mutateAsync({ name: file.name, bytes })) as { text: string };
      const text = (res?.text ?? '').trim();
      if (!text) {
        alert(t('ai.errors.failedToExtractText'));
        return;
      }
      setResumeText(text);
      setResumeFileName(file.name);
    } catch (err) {
      alert(err instanceof Error ? err.message : t('ai.errors.failedToUploadResume'));
    } finally {
      setUploading(false);
    }
  };

  const removeResume = () => {
    setResumeText('');
    setResumeFileName('');
  };

  const copyMessage = async (messageId: string, content: string) => {
    try {
      await navigator.clipboard.writeText(content);
      setCopiedMessageId(messageId);
      setTimeout(() => setCopiedMessageId(null), 1500);
    } catch {
      // Ignore copy errors
    }
  };

  // Auto-scroll on new content
  useEffect(() => {
    scrollRef.current?.scrollTo({ top: scrollRef.current.scrollHeight, behavior: 'smooth' });
  }, [messages]);

  const send = async () => {
    const text = input.trim();
    if (!text || streaming) return;
    setInput('');
    const userMsg: Msg = { id: crypto.randomUUID(), role: 'user', content: text };
    setMessages((prev) => [...prev, userMsg]);
    setStreaming(true);

    try {
      const locale = (['en', 'de'].includes(i18n.language) ? i18n.language : 'en') as 'en' | 'de';

      // Temporarily disable system prompt for testing
      const systemPrompt = 'You are a helpful AI assistant.';

      const history = [...messages, userMsg]
        .filter((m) => m.content)
        .map((m) => ({ role: m.role, content: m.content }));

      const contextMessages: Array<{ role: 'user' | 'assistant' | 'system'; content: string }> = [
        { role: 'system', content: systemPrompt },
        ...history,
      ];

      const res = (await generateAI.mutateAsync({
        model: selectedModel.current,
        messages: contextMessages,
        locale,
        ...(generateConfig.current.provider !== 'ollama'
          ? { provider: generateConfig.current.provider, baseUrl: generateConfig.current.baseUrl }
          : {}),
      })) as { jobId: string };

      activeJobRef.current = res.jobId;
    } catch {
      setMessages((prev) => [
        ...prev,
        {
          id: crypto.randomUUID(),
          role: 'assistant',
          content: t('ai.errors.failedToReachAI'),
        },
      ]);
      setStreaming(false);
    }
  };

  return {
    // State
    messages,
    input,
    setInput,
    streaming,
    isInputFocused,
    setIsInputFocused,
    resumeText,
    resumeFileName,
    uploading,
    copiedMessageId,
    scrollRef,
    fileInputRef,

    // Setters for external dependencies
    setSelectedModel: (model: string) => {
      selectedModel.current = model;
    },
    setGenerateConfig: (config: { provider: string; baseUrl: string | undefined }) => {
      generateConfig.current = config;
    },

    // Actions
    send,
    handleResumeUpload,
    removeResume,
    copyMessage,
  };
}
