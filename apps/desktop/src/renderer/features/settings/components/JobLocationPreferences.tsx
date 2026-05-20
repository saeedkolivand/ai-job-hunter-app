import { MapPin, Plus, X } from 'lucide-react';
import { AnimatePresence, motion } from 'motion/react';
import { useRef, useState } from 'react';
import { createPortal } from 'react-dom';

import { Button, GlassCard, Input } from '@ajh/ui';

import { useTranslation } from '@/lib/i18n';
import { transition } from '@/lib/motion';
import { useLocation, usePreferencesStore } from '@/store/preferences-store';

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
  const location = useLocation();
  const setLocation = usePreferencesStore((state) => state.setLocation);
  const [inputValue, setInputValue] = useState('');
  const [showSuggestions, setShowSuggestions] = useState(false);
  const [recentLocations, setRecentLocations] = useState<string[]>([]);
  const inputRef = useRef<HTMLInputElement>(null);
  const [dropdownPosition, setDropdownPosition] = useState<{
    top: number;
    left: number;
    width: number;
  } | null>(null);

  const filteredSuggestions = COMMON_LOCATIONS.filter(
    (loc) => loc.toLowerCase().includes(inputValue.toLowerCase()) && inputValue.length > 0
  );

  const handleAddLocation = (loc: string) => {
    if (!location) {
      setLocation({ city: loc });
    } else if (!location.city) {
      setLocation({ ...location, city: loc });
    }
    setInputValue('');
    setShowSuggestions(false);

    // Add to recent locations
    if (!recentLocations.includes(loc)) {
      setRecentLocations([loc, ...recentLocations.slice(0, 4)]);
    }
  };

  const handleRemoveLocation = () => {
    setLocation(undefined);
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
        <div className="text-xs font-medium uppercase tracking-[0.16em] text-foreground/40">
          {t('settings.location.title')}
        </div>
      </div>

      <p className="mb-4 text-sm text-foreground/55">{t('settings.location.description')}</p>

      {/* Current Location Display */}
      {location?.city && (
        <motion.div
          initial={{ opacity: 0, y: -10 }}
          animate={{ opacity: 1, y: 0 }}
          exit={{ opacity: 0, y: -10 }}
          className="mb-4 flex items-center gap-2 rounded-lg bg-white/5 px-4 py-3"
        >
          <MapPin size={16} className="text-brand-soft" />
          <span className="flex-1 text-sm text-foreground">{location.city}</span>
          <Button
            variant="ghost"
            size="sm"
            onClick={handleRemoveLocation}
            className="!bg-transparent hover:bg-white/5"
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
                setTimeout(updateDropdownPosition, 0);
              }
            }}
            onFocus={() => {
              setShowSuggestions(inputValue.length > 0);
              if (inputValue.length > 0) {
                updateDropdownPosition();
              }
            }}
            onBlur={() => setTimeout(() => setShowSuggestions(false), 150)}
            placeholder={t('settings.location.searchPlaceholder')}
            className="flex-1"
          />
          <Button
            variant="glass"
            size="md"
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
                className="fixed overflow-hidden rounded-xl border border-white/10 bg-secondary shadow-xl"
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
                      className="w-full flex items-center gap-3 rounded-lg px-3 py-2 text-sm text-foreground/70 hover:bg-white/5 hover:text-foreground transition-colors h-auto bg-transparent border-transparent"
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
          <div className="mb-2 text-xs font-medium uppercase tracking-[0.16em] text-foreground/40">
            {t('settings.location.recent')}
          </div>
          <div className="flex flex-wrap gap-2">
            {recentLocations.map((loc) => (
              <Button
                key={loc}
                variant="ghost"
                size="sm"
                onClick={() => handleAddLocation(loc)}
                className="!bg-transparent hover:bg-white/5"
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
