import { Code2, Plus, Search, X } from 'lucide-react';
import { AnimatePresence, motion } from 'motion/react';
import { useEffect, useRef, useState } from 'react';
import { createPortal } from 'react-dom';

import { Button, GlassCard, Input } from '@ajh/ui';

import { cn } from '@/lib/cn';
import { useTranslation } from '@/lib/i18n';
import { transition } from '@/lib/motion';
import { useJobPreferences, useSetJobPreferences } from '@/services';

const COMMON_TECH = [
  { name: 'JavaScript', category: 'language' },
  { name: 'TypeScript', category: 'language' },
  { name: 'Python', category: 'language' },
  { name: 'React', category: 'framework' },
  { name: 'Vue', category: 'framework' },
  { name: 'Angular', category: 'framework' },
  { name: 'Node.js', category: 'framework' },
  { name: 'PostgreSQL', category: 'database' },
  { name: 'MongoDB', category: 'database' },
  { name: 'Redis', category: 'database' },
  { name: 'Docker', category: 'tool' },
  { name: 'Kubernetes', category: 'tool' },
  { name: 'Git', category: 'tool' },
  { name: 'AWS', category: 'tool' },
  { name: 'GraphQL', category: 'language' },
];

const CATEGORY_COLORS: Record<string, string> = {
  language: 'bg-blue-500/20 text-blue-400',
  framework: 'bg-purple-500/20 text-purple-400',
  database: 'bg-green-500/20 text-green-400',
  tool: 'bg-orange-500/20 text-orange-400',
  other: 'bg-gray-500/20 text-gray-400',
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
      !techStack.some((item) => item.name === tech.name)
  );

  const handleAddTech = (name: string, category: string) => {
    setJobPreferences.mutate({
      ...jobPrefs,
      techStack: [...techStack, { name, category }],
    });
    setInputValue('');
    setShowSuggestions(false);
  };

  const handleRemoveTech = (name: string) => {
    setJobPreferences.mutate({
      ...jobPrefs,
      techStack: techStack.filter((item) => item.name !== name),
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
        <div className="text-xs font-medium uppercase tracking-[0.16em] text-foreground/40">
          Tech Stack
        </div>
      </div>

      <p className="mb-4 text-sm text-foreground/55">
        Add your technical skills to personalize job recommendations and AI analysis.
      </p>

      {/* Current Tech Stack */}
      {techStack.length > 0 && (
        <div className="mb-4 flex flex-wrap gap-2">
          {techStack.map((item) => (
            <motion.div
              key={item.name}
              initial={{ opacity: 0, scale: 0.9 }}
              animate={{ opacity: 1, scale: 1 }}
              exit={{ opacity: 0, scale: 0.9 }}
              className="flex items-center gap-2 rounded-full bg-white/5 px-3 py-1.5 text-sm"
            >
              <Code2 size={14} className="text-foreground/40" />
              <span className="text-foreground">{item.name}</span>
              <Button
                onClick={() => handleRemoveTech(item.name)}
                className="ml-1 rounded-full p-0.5 hover:bg-white/10 transition-colors h-auto bg-transparent border-transparent"
              >
                <X size={12} className="text-foreground/40 hover:text-foreground" />
              </Button>
            </motion.div>
          ))}
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
              placeholder={t('settings.techStack.searchPlaceholder')}
              className="pl-10"
            />
          </div>
          <Button
            variant="glass"
            size="md"
            onClick={() => {
              const match = COMMON_TECH.find(
                (t) => t.name.toLowerCase() === inputValue.toLowerCase()
              );
              if (match && !techStack.some((item) => item.name === match.name)) {
                handleAddTech(match.name, match.category);
              }
            }}
            disabled={
              !inputValue ||
              techStack.some((item) => item.name.toLowerCase() === inputValue.toLowerCase())
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
              className="overflow-hidden rounded-xl border border-white/10 bg-secondary shadow-xl"
            >
              <div className="max-h-48 overflow-y-auto px-1 py-1">
                {filteredSuggestions.map((suggestion) => (
                  <Button
                    key={suggestion.name}
                    onClick={() => handleAddTech(suggestion.name, suggestion.category)}
                    className="flex w-full items-center gap-3 rounded-lg px-3 py-2 text-sm text-foreground/70 hover:bg-white/5 hover:text-foreground transition-colors h-auto bg-transparent border-transparent"
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
          <div className="mb-2 text-xs font-medium uppercase tracking-[0.16em] text-foreground/40">
            {t('settings.techStack.popular')}
          </div>
          <div className="flex flex-wrap gap-2">
            {COMMON_TECH.slice(0, 6).map((tech) => (
              <Button
                key={tech.name}
                variant="ghost"
                size="sm"
                onClick={() => handleAddTech(tech.name, tech.category)}
                className="!bg-transparent hover:bg-white/5"
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
