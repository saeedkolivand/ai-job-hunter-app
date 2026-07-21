import { Code2, Plus, Search, X } from 'lucide-react';
import { AnimatePresence, motion } from 'motion/react';
import { useEffect, useRef, useState } from 'react';
import { createPortal } from 'react-dom';

import { useTranslation } from '@ajh/translations';
import { Button, cn, GlassCard, Input, transition } from '@ajh/ui';

import { COMMON_TECH } from '@/constants/tech-stack';
import { useJobPreferences, useSetJobPreferences } from '@/services';

const CATEGORY_COLORS: Record<string, string> = {
  language: 'bg-blue-500/20 text-blue-400',
  framework: 'bg-purple-500/20 text-purple-400',
  database: 'bg-green-500/20 text-green-400',
  tool: 'bg-orange-500/20 text-orange-400',
  other: 'bg-gray-500/20 text-foreground/50',
};

export function TechStackPreferences() {
  const { t } = useTranslation();
  const { data: jobPrefs } = useJobPreferences();
  const setJobPreferences = useSetJobPreferences();
  const techStack = jobPrefs?.techStack || [];
  const [inputValue, setInputValue] = useState('');
  const [showSuggestions, setShowSuggestions] = useState(false);
  const [position, setPosition] = useState({ top: 0, left: 0, width: 0 });
  const inputRef = useRef<HTMLDivElement>(null);
  const suggestionsRef = useRef<HTMLDivElement>(null);

  const filteredSuggestions = COMMON_TECH.filter(
    (tech) =>
      tech.name.toLowerCase().includes(inputValue.toLowerCase()) &&
      inputValue.length > 0 &&
      !techStack.some((item: { name: string }) => item.name === tech.name)
  );

  // Guard the pre-load window: `jobPrefs` is undefined until the query resolves,
  // and a full-row `{...undefined, techStack}` write would NULL every other
  // column (location, countryCode, salaryExpectation, extraAgencyCompanies).
  const handleAddTech = (name: string, category: string) => {
    if (!jobPrefs) return;
    setJobPreferences.mutate({
      ...jobPrefs,
      techStack: [...techStack, { name, category }],
    });
    setInputValue('');
    setShowSuggestions(false);
  };

  const addTech = (raw: string) => {
    if (!jobPrefs) return;
    const trimmed = raw.trim();
    if (!trimmed) return;
    const isDup = techStack.some(
      (item: { name: string }) => item.name.toLowerCase() === trimmed.toLowerCase()
    );
    if (isDup) return;
    const match = COMMON_TECH.find((tch) => tch.name.toLowerCase() === trimmed.toLowerCase());
    setJobPreferences.mutate({
      ...jobPrefs,
      techStack: [
        ...techStack,
        { name: match?.name ?? trimmed, category: match?.category ?? 'other' },
      ],
    });
    setInputValue('');
    setShowSuggestions(false);
  };

  const handleRemoveTech = (name: string) => {
    if (!jobPrefs) return;
    setJobPreferences.mutate({
      ...jobPrefs,
      techStack: techStack.filter((item: { name: string }) => item.name !== name),
    });
  };

  useEffect(() => {
    if (showSuggestions && inputRef.current) {
      const rect = inputRef.current.getBoundingClientRect();
      setPosition({
        top: rect.bottom + 8,
        left: rect.left,
        width: rect.width,
      });
    }
  }, [showSuggestions]);

  const handleClickOutside = (e: MouseEvent) => {
    if (
      suggestionsRef.current &&
      !suggestionsRef.current.contains(e.target as Node) &&
      inputRef.current &&
      !inputRef.current.contains(e.target as Node)
    ) {
      setShowSuggestions(false);
    }
  };

  useEffect(() => {
    if (showSuggestions) {
      document.addEventListener('mousedown', handleClickOutside);
      return () => document.removeEventListener('mousedown', handleClickOutside);
    }
  }, [showSuggestions]);

  return (
    <GlassCard>
      <div className="mb-4 flex items-center justify-between">
        <div className="text-xs font-semibold uppercase tracking-[0.16em] text-foreground/55">
          {t('settings.techStack.title')}
        </div>
      </div>

      <p className="mb-4 text-sm text-foreground/55">{t('settings.techStack.description')}</p>

      {/* Current Tech Stack */}
      {techStack.length > 0 && (
        <div className="mb-4 flex flex-wrap gap-2">
          <AnimatePresence>
            {techStack.map((item: { name: string; category: string }) => (
              <motion.div
                key={item.name}
                layout
                initial={{ opacity: 0, scale: 0.9 }}
                animate={{ opacity: 1, scale: 1 }}
                exit={{ opacity: 0, scale: 0.9 }}
                className="flex items-center gap-2 rounded-full bg-foreground/[0.06] px-3 py-1.5 text-sm"
              >
                <Code2 size={14} className="text-foreground/40" />
                <span className="text-foreground">{item.name}</span>
                <Button
                  onClick={() => handleRemoveTech(item.name)}
                  className="ml-1 rounded-full p-0.5 hover:bg-foreground/10 transition-colors h-auto bg-transparent border-transparent"
                >
                  <X size={12} className="text-foreground/40 hover:text-foreground" />
                </Button>
              </motion.div>
            ))}
          </AnimatePresence>
        </div>
      )}

      {/* Add Tech Input */}
      <div ref={inputRef}>
        <div className="flex items-center gap-2">
          <div className="relative flex-1">
            <Search
              size={16}
              className="absolute left-3 top-1/2 -translate-y-1/2 text-foreground/40"
            />
            <Input
              value={inputValue}
              onChange={(e) => {
                setInputValue(e.target.value);
                setShowSuggestions(e.target.value.length > 0);
              }}
              onFocus={() => setShowSuggestions(inputValue.length > 0)}
              onKeyDown={(e) => {
                if (e.key === 'Enter') {
                  e.preventDefault();
                  addTech(inputValue);
                }
              }}
              placeholder={t('settings.techStack.searchPlaceholder')}
              className="pl-10"
            />
          </div>
          <Button
            variant="glass"
            size="md"
            onClick={() => addTech(inputValue)}
            disabled={
              !inputValue ||
              techStack.some(
                (item: { name: string }) => item.name.toLowerCase() === inputValue.toLowerCase()
              )
            }
          >
            <Plus size={16} />
          </Button>
        </div>
      </div>

      {/* Autocomplete Suggestions */}
      {showSuggestions &&
        filteredSuggestions.length > 0 &&
        createPortal(
          <AnimatePresence>
            <motion.div
              ref={suggestionsRef}
              initial={{ opacity: 0, y: -8, scale: 0.98 }}
              animate={{ opacity: 1, y: 0, scale: 1 }}
              exit={{ opacity: 0, y: -8, scale: 0.98 }}
              transition={transition.fast}
              style={{
                position: 'fixed',
                top: position.top,
                left: position.left,
                width: position.width,
                zIndex: 9999,
              }}
              className="overflow-hidden rounded-xl border border-foreground/10 bg-secondary shadow-xl"
            >
              <div className="max-h-48 overflow-y-auto px-1 py-1">
                {filteredSuggestions.map((suggestion) => (
                  <Button
                    key={suggestion.name}
                    onClick={() => handleAddTech(suggestion.name, suggestion.category)}
                    className="flex w-full items-center gap-3 rounded-lg px-3 py-2 text-sm text-foreground/70 hover:bg-foreground/[0.06] hover:text-foreground transition-colors h-auto bg-transparent border-transparent"
                  >
                    <Code2 size={14} className="text-foreground/40" />
                    <span className="flex-1 text-left">{suggestion.name}</span>
                    <span
                      className={cn(
                        'rounded-full px-2 py-0.5 text-xs',
                        CATEGORY_COLORS[suggestion.category] || CATEGORY_COLORS.other
                      )}
                    >
                      {suggestion.category}
                    </span>
                  </Button>
                ))}
              </div>
            </motion.div>
          </AnimatePresence>,
          document.body
        )}

      {/* Quick Add Suggestions */}
      {techStack.length === 0 && (
        <div className="mt-4">
          <div className="mb-2 text-xs font-semibold uppercase tracking-[0.16em] text-foreground/55">
            {t('settings.techStack.popular')}
          </div>
          <div className="flex flex-wrap gap-2">
            {COMMON_TECH.slice(0, 6).map((tech) => (
              <Button
                key={tech.name}
                variant="ghost"
                onClick={() => handleAddTech(tech.name, tech.category)}
                className="!bg-transparent hover:bg-foreground/[0.06]"
              >
                <Code2 size={12} className="text-foreground/40" />
                {tech.name}
              </Button>
            ))}
          </div>
        </div>
      )}
    </GlassCard>
  );
}
