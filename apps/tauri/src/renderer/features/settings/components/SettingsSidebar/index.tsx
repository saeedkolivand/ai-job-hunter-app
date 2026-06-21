import { ChevronRight, SearchX } from 'lucide-react';
import { motion } from 'motion/react';
import { useCallback, useEffect, useId, useRef, useState } from 'react';

import { useTranslation } from '@ajh/translations';
import { Button, cn, EmptyState, Input, NavPill, transition } from '@ajh/ui';

import { NAV_GROUPS, type NavGroup, type SectionId } from '@/features/settings/constants';
import { matchEntries, type SearchResult } from '@/features/settings/lib/search';

/** Map SectionId → its i18n label key, derived from the canonical NAV_GROUPS. */
const SECTION_LABEL_KEYS: Record<SectionId, string> = Object.fromEntries(
  NAV_GROUPS.flatMap((g) => g.items.map((item) => [item.id, item.label]))
) as Record<SectionId, string>;

interface Props {
  navGroups: NavGroup[];
  activeSection: SectionId;
  onSectionChange: (id: SectionId) => void;
  onResultSelect?: (section: SectionId, anchor: string) => void;
}

export function SettingsSidebar({
  navGroups,
  activeSection,
  onSectionChange,
  onResultSelect,
}: Props) {
  const { t } = useTranslation();
  const [query, setQuery] = useState('');
  const [highlightedIdx, setHighlightedIdx] = useState(0);
  const inputRef = useRef<HTMLInputElement>(null);
  const listRef = useRef<HTMLUListElement>(null);
  const listboxId = useId();

  const results: SearchResult[] = query.trim() ? matchEntries(query, t, SECTION_LABEL_KEYS) : [];

  const isSearching = query.trim().length > 0;

  // Reset highlight index when results change.
  useEffect(() => {
    setHighlightedIdx(0);
  }, [query]);

  // Ctrl/Cmd+F to focus the search field while on the Settings page.
  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if ((e.ctrlKey || e.metaKey) && e.key === 'f') {
        e.preventDefault();
        inputRef.current?.focus();
      }
    };
    window.addEventListener('keydown', handler);
    return () => window.removeEventListener('keydown', handler);
  }, []);

  const handleSelect = useCallback(
    (result: SearchResult) => {
      setQuery('');
      if (onResultSelect) {
        onResultSelect(result.section, result.anchor);
      } else {
        onSectionChange(result.section);
      }
    },
    [onResultSelect, onSectionChange]
  );

  const handleKeyDown = (e: React.KeyboardEvent<HTMLInputElement>) => {
    if (!isSearching) return;

    if (e.key === 'Escape') {
      setQuery('');
      inputRef.current?.blur();
      return;
    }

    if (results.length === 0) return;

    if (e.key === 'ArrowDown') {
      e.preventDefault();
      setHighlightedIdx((i) => (i + 1) % results.length);
    } else if (e.key === 'ArrowUp') {
      e.preventDefault();
      setHighlightedIdx((i) => (i - 1 + results.length) % results.length);
    } else if (e.key === 'Enter') {
      e.preventDefault();
      const result = results[highlightedIdx];
      if (result) handleSelect(result);
    }
  };

  // Keep highlighted item scrolled into view inside the listbox.
  useEffect(() => {
    if (!listRef.current) return;
    const item = listRef.current.children[highlightedIdx] as HTMLElement | undefined;
    item?.scrollIntoView({ block: 'nearest' });
  }, [highlightedIdx]);

  const highlightedId =
    isSearching && results.length > 0 ? `${listboxId}-option-${highlightedIdx}` : undefined;

  // Announce result count to screen readers on query change.
  const liveText = isSearching
    ? results.length > 0
      ? t('settings.search.resultCount', { count: results.length })
      : t('settings.search.noResultsAria', { query: query.trim() })
    : '';

  return (
    <aside className="flex w-44 lg:w-56 shrink-0 flex-col gap-4 overflow-y-auto border-foreground/10 px-3 py-8">
      {/* Screen-reader live region for result count (item #3) */}
      <span className="sr-only" aria-live="polite" aria-atomic="true">
        {liveText}
      </span>

      {/* ── Search field ─────────────────────────────────────────────────────── */}
      <div className="px-0.5">
        <Input
          ref={inputRef}
          type="text"
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          onKeyDown={handleKeyDown}
          placeholder={t('settings.search.placeholder')}
          aria-label={t('settings.search.ariaLabel')}
          aria-expanded={isSearching && results.length > 0}
          aria-controls={isSearching && results.length > 0 ? listboxId : undefined}
          aria-activedescendant={highlightedId}
          aria-autocomplete="list"
          role="combobox"
          className="h-8 text-xs"
        />
      </div>

      {/* ── Search results ────────────────────────────────────────────────────── */}
      {isSearching ? (
        <div className="flex flex-col gap-0.5">
          {results.length === 0 ? (
            <EmptyState
              icon={SearchX}
              title={t('settings.search.noResults', { query: query.trim() })}
              className="py-6 text-xs"
            />
          ) : (
            <ul
              ref={listRef}
              id={listboxId}
              role="listbox"
              aria-label={t('settings.search.resultsLabel')}
              className="flex flex-col gap-0.5"
            >
              {results.map((result, idx) => {
                const isHighlighted = idx === highlightedIdx;
                const optionId = `${listboxId}-option-${idx}`;
                return (
                  <li key={result.id} id={optionId} role="option" aria-selected={isHighlighted}>
                    <Button
                      variant="unstyled"
                      tabIndex={-1}
                      onClick={() => handleSelect(result)}
                      className={cn(
                        'flex w-full flex-col items-start rounded-xl px-3 py-2 text-left transition-colors duration-100',
                        isHighlighted
                          ? 'bg-brand/[0.12] text-foreground ring-1 ring-brand/50'
                          : 'text-foreground/55 hover:bg-foreground/[0.05] hover:text-foreground/80'
                      )}
                    >
                      <span className="text-xs font-semibold leading-tight">{result.title}</span>
                      <span className="mt-0.5 max-w-full truncate text-[10px] text-foreground/55">
                        {result.sectionLabel}
                      </span>
                    </Button>
                  </li>
                );
              })}
            </ul>
          )}
        </div>
      ) : (
        /* ── Nav groups (default) ────────────────────────────────────────────── */
        <>
          {navGroups.map((group) => (
            <div key={group.label}>
              <div className="mb-1.5 px-3 text-[10px] font-semibold uppercase tracking-widest text-foreground/55">
                {group.label}
              </div>
              <nav className="flex flex-col gap-1">
                {group.items.map(({ id, label, icon: Icon }) => {
                  const active = activeSection === id;
                  return (
                    <div key={id} className="relative">
                      {active && <NavPill layoutId="settings-pill" />}
                      <Button
                        variant="unstyled"
                        type="button"
                        aria-current={active ? 'page' : undefined}
                        onClick={() => onSectionChange(id)}
                        className={cn(
                          'group relative flex w-full cursor-pointer items-center gap-2.5 rounded-xl px-3 py-2 text-left text-sm transition-colors duration-150',
                          active
                            ? 'text-foreground'
                            : 'text-foreground/45 hover:bg-foreground/[0.05] hover:text-foreground/75'
                        )}
                      >
                        <Icon
                          size={15}
                          className={cn(
                            'shrink-0 transition-colors duration-150',
                            active
                              ? 'text-brand-soft'
                              : 'text-foreground/35 group-hover:text-foreground/55'
                          )}
                        />
                        <span className="flex-1 font-medium">{label}</span>
                        {active && (
                          <motion.span
                            layoutId="settings-chevron"
                            transition={transition.spring}
                            className="text-brand-soft"
                          >
                            <ChevronRight size={12} />
                          </motion.span>
                        )}
                      </Button>
                    </div>
                  );
                })}
              </nav>
            </div>
          ))}
        </>
      )}
    </aside>
  );
}
