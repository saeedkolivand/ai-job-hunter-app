import { MapPin, Plus, X } from 'lucide-react';
import { AnimatePresence, motion } from 'motion/react';
import { useEffect, useRef, useState } from 'react';
import { createPortal } from 'react-dom';

import { useTranslation } from '@ajh/translations';
import { Button, GlassCard, Input, transition } from '@ajh/ui';

import { useJobPreferences, useSetJobPreferences } from '@/services';
import { usePreferencesStore, useRecentLocations } from '@/store/preferences-store';

const COMMON_LOCATIONS = [
  'San Francisco, CA',
  'New York, NY',
  'London, UK',
  'Berlin, Germany',
  'Remote',
  'Toronto, Canada',
  'Sydney, Australia',
  'Amsterdam, Netherlands',
];

export function JobLocationPreferences() {
  const { t } = useTranslation();
  const { data: jobPrefs } = useJobPreferences();
  const setJobPreferences = useSetJobPreferences();
  const [inputValue, setInputValue] = useState('');
  const [showSuggestions, setShowSuggestions] = useState(false);
  // Persisted to the preferences store so "Recent" survives tab switches/restarts.
  const recentLocations = useRecentLocations();
  const addRecentLocation = usePreferencesStore((s) => s.addRecentLocation);
  const inputRef = useRef<HTMLInputElement>(null);
  const [dropdownPosition, setDropdownPosition] = useState<{
    top: number;
    left: number;
    width: number;
  } | null>(null);

  // The blur-hide and position-measure both run on a timer that calls setState.
  // Keep their ids so a mid-timeout unmount can cancel them — a stray setState
  // after jsdom teardown throws `window is not defined` and fails the whole run.
  const blurTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const positionTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  useEffect(
    () => () => {
      if (blurTimerRef.current) clearTimeout(blurTimerRef.current);
      if (positionTimerRef.current) clearTimeout(positionTimerRef.current);
    },
    []
  );

  const filteredSuggestions = COMMON_LOCATIONS.filter(
    (loc) => loc.toLowerCase().includes(inputValue.toLowerCase()) && inputValue.length > 0
  );

  const handleAddLocation = (loc: string) => {
    // Guard the pre-load window: `jobPrefs` is undefined until the query
    // resolves, and a full-row `{...undefined, location}` write would NULL every
    // other column (techStack, countryCode, salaryExpectation, extraAgencyCompanies).
    if (!jobPrefs) return;
    setJobPreferences.mutate({
      ...jobPrefs,
      location: loc,
    });
    setInputValue('');
    setShowSuggestions(false);

    // Persist to recent locations (dedup + cap handled by the store action).
    addRecentLocation(loc);
  };

  const handleRemoveLocation = () => {
    if (!jobPrefs) return;
    setJobPreferences.mutate({
      ...jobPrefs,
      location: undefined,
    });
  };

  const updateDropdownPosition = () => {
    if (inputRef.current) {
      const rect = inputRef.current.getBoundingClientRect();
      setDropdownPosition({
        top: rect.bottom + 8,
        left: rect.left,
        width: rect.width,
      });
    }
  };

  return (
    <GlassCard>
      <div className="mb-4 flex items-center justify-between">
        <div className="text-xs font-semibold uppercase tracking-[0.16em] text-foreground/55">
          {t('settings.location.title')}
        </div>
      </div>

      <p className="mb-4 text-sm text-foreground/55">{t('settings.location.description')}</p>

      {/* Current Location Display */}
      {jobPrefs?.location && (
        <motion.div
          initial={{ opacity: 0, y: -10 }}
          animate={{ opacity: 1, y: 0 }}
          exit={{ opacity: 0, y: -10 }}
          className="mb-4 flex items-center gap-2 rounded-lg bg-foreground/[0.06] px-4 py-3"
        >
          <MapPin size={16} className="text-brand-soft" />
          <span className="flex-1 text-sm text-foreground">{jobPrefs.location}</span>
          <Button
            variant="ghost"
            onClick={handleRemoveLocation}
            className="h-8 w-8 p-0 !bg-transparent hover:bg-foreground/[0.06]"
          >
            <X size={14} />
          </Button>
        </motion.div>
      )}

      {/* Location Input */}
      <div className="relative">
        <div className="flex items-center gap-2">
          <Input
            ref={inputRef}
            value={inputValue}
            onChange={(e) => {
              setInputValue(e.target.value);
              setShowSuggestions(e.target.value.length > 0);
              if (e.target.value.length > 0) {
                if (positionTimerRef.current) clearTimeout(positionTimerRef.current);
                positionTimerRef.current = setTimeout(updateDropdownPosition, 0);
              }
            }}
            onFocus={() => {
              setShowSuggestions(inputValue.length > 0);
              if (inputValue.length > 0) {
                updateDropdownPosition();
              }
            }}
            onBlur={() => {
              // 150ms delay so a click on a suggestion registers before the
              // dropdown hides; the ref lets an unmount cancel the pending hide.
              blurTimerRef.current = setTimeout(() => setShowSuggestions(false), 150);
            }}
            placeholder={t('settings.location.searchPlaceholder')}
            className="flex-1"
          />
          <Button
            variant="glass"
            size="md"
            className="h-8 w-8 p-0"
            onClick={() => inputValue && handleAddLocation(inputValue)}
            disabled={!inputValue}
          >
            <Plus size={16} />
          </Button>
        </div>

        {/* Autocomplete Suggestions - Portal */}
        {createPortal(
          <AnimatePresence>
            {showSuggestions && filteredSuggestions.length > 0 && dropdownPosition && (
              <motion.div
                initial={{ opacity: 0, y: -8, scale: 0.98 }}
                animate={{ opacity: 1, y: 0, scale: 1 }}
                exit={{ opacity: 0, y: -8, scale: 0.98 }}
                transition={transition.fast}
                className="fixed overflow-hidden rounded-xl border border-foreground/10 bg-secondary shadow-xl"
                style={{
                  top: `${dropdownPosition.top}px`,
                  left: `${dropdownPosition.left}px`,
                  width: `${dropdownPosition.width}px`,
                  zIndex: 10000,
                }}
              >
                <div className="max-h-48 overflow-y-auto px-1 py-1">
                  {filteredSuggestions.map((suggestion) => (
                    <Button
                      key={suggestion}
                      onClick={() => handleAddLocation(suggestion)}
                      className="w-full flex items-center gap-3 rounded-lg px-3 py-2 text-sm text-foreground/70 hover:bg-foreground/[0.06] hover:text-foreground transition-colors h-auto bg-transparent border-transparent"
                    >
                      <MapPin size={14} className="text-foreground/40" />
                      {suggestion}
                    </Button>
                  ))}
                </div>
              </motion.div>
            )}
          </AnimatePresence>,
          document.body
        )}
      </div>

      {/* Recent Locations */}
      {recentLocations.length > 0 && (
        <div className="mt-4">
          <div className="mb-2 text-xs font-semibold uppercase tracking-[0.16em] text-foreground/55">
            {t('settings.location.recent')}
          </div>
          <div className="flex flex-wrap gap-2">
            {recentLocations.map((loc) => (
              <Button
                key={loc}
                variant="ghost"
                onClick={() => handleAddLocation(loc)}
                className="!bg-transparent hover:bg-foreground/[0.06]"
              >
                <MapPin size={12} className="text-foreground/40" />
                {loc}
              </Button>
            ))}
          </div>
        </div>
      )}
    </GlassCard>
  );
}
