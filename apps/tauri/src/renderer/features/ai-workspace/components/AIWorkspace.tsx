import { Check, ChevronDown, Copy, RefreshCw, Send, Sparkles, Upload, X } from 'lucide-react';
import { motion } from 'motion/react';
import { useEffect, useRef, useState } from 'react';
import { useQueryClient } from '@tanstack/react-query';

import { buildWorkspaceSystemPrompt } from '@ajh/prompts';
import type { AiStreamChunk, JobEvent } from '@ajh/shared';
import { Button, Input, MarkdownMessage } from '@ajh/ui';

import i18n from '@/i18n';
import { cn } from '@/lib/cn';
import { useTranslation } from '@/lib/i18n';
import { transition } from '@/lib/motion';
import {
  useAIModels,
  useAIStream,
  useExtractText,
  useGenerateAI,
  useGetOrCreateConversation,
  useJobEvents,
} from '@/services';
import { keys } from '@/services/query-client';
import { useAIModel, usePreferencesStore } from '@/store/preferences-store';
import type { Model } from '@/types';

const ACCEPTED_EXTS = ['pdf', 'docx', 'txt', 'md', 'markdown'] as const;
const ACCEPT_ATTR = '.pdf,.docx,.txt,.md,.markdown';
const MAX_BYTES = 25 * 1024 * 1024;

interface Msg {
  id: string;
  role: 'user' | 'assistant';
  content: string;
  displayedContent?: string;
  jobId?: string;
  isStreaming?: boolean;
}

export function AIWorkspace() {
  const { t } = useTranslation();
  const [messages, setMessages] = useState<Msg[]>([]);
  const [input, setInput] = useState('');
  const [streaming, setStreaming] = useState(false);
  const [isInputFocused, setIsInputFocused] = useState(false);
  const [resumeText, setResumeText] = useState<string>('');
  const [resumeFileName, setResumeFileName] = useState<string>('');
  const [uploading, setUploading] = useState(false);
  const [copiedMessageId, setCopiedMessageId] = useState<string | null>(null);
  const [showModelPicker, setShowModelPicker] = useState(false);
  const activeJobRef = useRef<string | null>(null);
  const scrollRef = useRef<HTMLDivElement>(null);
  const fileInputRef = useRef<HTMLInputElement>(null);
  const aiModel = useAIModel();
  const { data: modelList = [], isFetching: loadingModels } = useAIModels();
  const models = modelList as Model[];
  const qc = useQueryClient();
  const setAIModel = usePreferencesStore((s) => s.setAIModel);

  const getOrCreateConversation = useGetOrCreateConversation();
  const extractText = useExtractText();
  const generateAI = useGenerateAI();

  // Load conversation once on mount — capture mutateAsync in a ref so the effect
  // doesn't need it as a dep (useMutation returns a new reference on every render)
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

  // Auto-scroll on new content.
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

      const systemPrompt = buildWorkspaceSystemPrompt({
        locale,
        resumeText: resumeText || undefined,
        modelName: aiModel?.defaultModel,
      });

      const history = [...messages, userMsg]
        .filter((m) => m.content)
        .map((m) => ({ role: m.role, content: m.content }));

      const contextMessages: Array<{ role: 'user' | 'assistant' | 'system'; content: string }> = [
        { role: 'system', content: systemPrompt },
        ...history,
      ];

      const res = (await generateAI.mutateAsync({
        model: aiModel?.defaultModel ?? '',
        messages: contextMessages,
        locale,
      })) as { jobId: string };

      activeJobRef.current = res.jobId;
      // Don't add empty message box - wait for first chunk of content
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

  return (
    <div className="flex h-full flex-col relative">
      {/* Floating model selector button */}
      <div className="absolute top-4 right-4 z-10">
        <div className="relative">
          <button
            onClick={() => setShowModelPicker(!showModelPicker)}
            className="flex items-center gap-2 rounded-lg border border-white/[0.08] bg-white/[0.03] px-3 py-1.5 text-xs text-foreground/70 hover:bg-white/[0.06] hover:text-foreground/90 transition-all backdrop-blur-sm"
          >
            <Sparkles size={12} className="text-brand-soft" />
            <span className="max-w-[120px] truncate">
              {aiModel?.defaultModel || 'Select model'}
            </span>
            <ChevronDown
              size={12}
              className={cn('transition-transform', showModelPicker && 'rotate-180')}
            />
          </button>

          {/* Model picker dropdown */}
          {showModelPicker && (
            <>
              {/* Backdrop to close dropdown */}
              <div className="fixed inset-0 z-20" onClick={() => setShowModelPicker(false)} />
              <div className="absolute right-0 top-full mt-2 w-64 rounded-xl border border-white/[0.08] bg-black/95 backdrop-blur-xl shadow-2xl z-30 overflow-hidden">
                {/* Refresh button */}
                <div className="border-b border-white/[0.06] p-2">
                  <button
                    onClick={() => void qc.invalidateQueries({ queryKey: keys.ai.models })}
                    disabled={loadingModels}
                    className="flex w-full items-center gap-2 rounded-lg px-3 py-2 text-xs text-foreground/60 hover:bg-white/[0.05] hover:text-foreground/80 transition-colors disabled:opacity-40"
                  >
                    <RefreshCw size={12} className={loadingModels ? 'animate-spin' : ''} />
                    Refresh models
                  </button>
                </div>

                {/* Model list */}
                <div className="max-h-80 overflow-y-auto p-2">
                  {models.length === 0 ? (
                    <div className="px-3 py-4 text-center text-xs text-foreground/40">
                      No models available
                    </div>
                  ) : (
                    models.map((model) => {
                      const isSelected = model.name === aiModel?.defaultModel;
                      return (
                        <button
                          key={model.name}
                          onClick={() => {
                            setAIModel({
                              defaultModel: model.name,
                              temperature: 0.7,
                              maxTokens: 2000,
                            });
                            setShowModelPicker(false);
                          }}
                          className={cn(
                            'flex w-full items-center gap-2 rounded-lg px-3 py-2 text-left text-xs transition-colors',
                            isSelected
                              ? 'bg-brand/20 text-brand-soft'
                              : 'text-foreground/70 hover:bg-white/[0.05] hover:text-foreground/90'
                          )}
                        >
                          {isSelected && <Check size={12} className="shrink-0" />}
                          <span className="flex-1 truncate">{model.name}</span>
                        </button>
                      );
                    })
                  )}
                </div>
              </div>
            </>
          )}
        </div>
      </div>

      <div ref={scrollRef} className="flex-1 overflow-y-auto px-8 py-6">
        {messages.length === 0 ? (
          <div className="flex h-full flex-col items-center justify-center text-center">
            <div className="glass-elevated mb-5 flex h-14 w-14 items-center justify-center rounded-2xl glow-subtle">
              <Sparkles size={22} className="text-brand-soft" />
            </div>
            <h2 className="text-gradient text-2xl font-semibold tracking-tight">{t('nav.ai')}</h2>
            <p className="mt-2 max-w-sm text-sm text-foreground/50">{t('ai.placeholder')}</p>
            <div className="mt-6 grid grid-cols-2 gap-2 max-w-lg">
              {[
                { q: 'How do I search for jobs on LinkedIn?', icon: '🔍' },
                { q: 'How do I tailor my resume for a role?', icon: '📝' },
                { q: 'What does the ATS score mean?', icon: '📊' },
                { q: 'How do I set up Autopilot?', icon: '🤖' },
              ].map(({ q, icon }) => (
                <Button
                  key={q}
                  onClick={() => setInput(q)}
                  className="flex items-start gap-2.5 rounded-xl border border-white/[0.07] bg-white/[0.02] px-3 py-2.5 text-left text-xs text-foreground/60 hover:border-brand/20 hover:bg-brand/5 hover:text-foreground/80 transition-all h-auto"
                >
                  <span className="text-base leading-none shrink-0">{icon}</span>
                  <span>{q}</span>
                </Button>
              ))}
            </div>
          </div>
        ) : (
          <div className="mx-auto flex max-w-3xl flex-col gap-4">
            {messages.map((m) => (
              <div key={m.id} className="flex flex-col gap-2">
                <div
                  className={cn(
                    'glass-card rounded-2xl px-4 py-3',
                    m.role === 'user'
                      ? 'self-end max-w-[80%] glow-subtle text-sm leading-relaxed'
                      : 'self-start max-w-[85%]'
                  )}
                >
                  {m.role === 'user' ? (
                    m.displayedContent || m.content
                  ) : (
                    <MarkdownMessage content={m.displayedContent || m.content} />
                  )}
                </div>
                {m.role === 'assistant' && !streaming && m.content && (
                  <div
                    className={cn(
                      'flex justify-end',
                      m.role === 'assistant' ? 'self-start max-w-[85%]' : ''
                    )}
                  >
                    <Button
                      onClick={() => void copyMessage(m.id, m.content)}
                      className="flex items-center gap-1.5 rounded-md bg-white/5 px-2.5 py-1 text-[11px] text-foreground/70 hover:text-foreground transition-colors h-auto"
                      aria-label={t('ai.copyMessage')}
                    >
                      {copiedMessageId === m.id ? (
                        <Check size={12} className="text-emerald-300" />
                      ) : (
                        <Copy size={12} />
                      )}
                      {copiedMessageId === m.id ? t('ai.copied') : t('ai.copy')}
                    </Button>
                  </div>
                )}
              </div>
            ))}

            {/* Simple Loading State */}
            {streaming && (
              <motion.div
                initial={{ opacity: 0, y: 20 }}
                animate={{ opacity: 1, y: 0 }}
                className="flex items-center gap-3 self-start max-w-[85%]"
              >
                <motion.div
                  animate={{
                    scale: [1, 1.2, 1],
                    opacity: [0.5, 1, 0.5],
                  }}
                  transition={transition.pulse}
                  className="flex h-8 w-8 items-center justify-center rounded-full bg-brand-soft/20"
                >
                  <Sparkles size={16} className="text-brand-soft" />
                </motion.div>
                <span className="text-xs text-foreground/50">{t('ai.processing')}</span>
              </motion.div>
            )}
          </div>
        )}
      </div>

      <div className="border-t border-white/5 p-4">
        {resumeFileName && (
          <div className="mx-auto mb-3 flex max-w-3xl items-center gap-2 rounded-lg border border-brand-soft/20 bg-brand-soft/5 px-3 py-2">
            <span className="flex-1 truncate text-xs text-brand-soft/90">{resumeFileName}</span>
            <Button
              onClick={removeResume}
              className="text-brand-soft/70 hover:text-brand-soft h-auto bg-transparent border-transparent p-0"
              aria-label={t('ai.removeResume')}
            >
              <X size={14} />
            </Button>
          </div>
        )}
        <motion.div
          className={cn(
            'glass-elevated mx-auto flex max-w-3xl items-center gap-2 rounded-2xl px-4 py-2.5',
            isInputFocused && 'glow-subtle'
          )}
          animate={{
            boxShadow: isInputFocused ? '0 0 20px rgba(192, 132, 252, 0.3)' : 'none',
          }}
          transition={transition.relaxed}
        >
          <Sparkles
            size={15}
            className={cn(
              'transition-colors',
              isInputFocused ? 'text-brand-soft' : 'text-foreground/40'
            )}
          />
          <input
            ref={fileInputRef}
            type="file"
            accept={ACCEPT_ATTR}
            className="hidden"
            onChange={(e) => {
              const file = e.target.files?.[0];
              if (file) void handleResumeUpload(file);
              e.target.value = '';
            }}
          />
          <Button
            type="button"
            onClick={() => fileInputRef.current?.click()}
            disabled={uploading}
            className={cn(
              'rounded-lg p-2 text-foreground/70 transition-colors hover:bg-white/10 disabled:opacity-40 h-auto bg-transparent border-transparent',
              uploading && 'cursor-wait'
            )}
            aria-label={t('ai.uploadResume')}
          >
            <Upload size={14} className={uploading ? 'animate-pulse' : ''} />
          </Button>
          <Input
            value={input}
            onChange={(e) => setInput(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === 'Enter' && !e.shiftKey) {
                e.preventDefault();
                void send();
              }
            }}
            onFocus={() => setIsInputFocused(true)}
            onBlur={() => setIsInputFocused(false)}
            placeholder={t('ai.placeholder')}
            disabled={streaming}
            variant="default"
            className="flex-1 bg-transparent"
          />
          <Button
            onClick={() => void send()}
            disabled={!input.trim() || streaming}
            className="rounded-lg bg-white/5 p-2 text-foreground/70 transition-colors hover:bg-white/10 disabled:opacity-40 h-auto border-transparent"
            aria-label={t('ai.send')}
          >
            <Send size={14} />
          </Button>
        </motion.div>
      </div>
    </div>
  );
}
