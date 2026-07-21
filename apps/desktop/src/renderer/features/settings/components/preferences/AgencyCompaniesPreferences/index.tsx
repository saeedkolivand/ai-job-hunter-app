import { Building2, Plus, X } from 'lucide-react';
import { AnimatePresence, motion } from 'motion/react';
import { useState } from 'react';

import { useTranslation } from '@ajh/translations';
import { Button, GlassCard, Input } from '@ajh/ui';

import { useJobPreferences, useSetExtraAgencyCompanies } from '@/services';

/**
 * Job preference editor for `extraAgencyCompanies` (ADR-029 §i) — company names
 * the user wants treated as recruiting/staffing agencies on top of the built-in
 * list. A tag-input: removable chips plus an add field. Writes go through the
 * single-column `useSetExtraAgencyCompanies` setter (never the full-row `set()`,
 * per the PR #695 stale-cache foot-gun).
 */
export function AgencyCompaniesPreferences() {
  const { t } = useTranslation();
  const { data: jobPrefs } = useJobPreferences();
  const setExtraAgencyCompanies = useSetExtraAgencyCompanies();
  const list = jobPrefs?.extraAgencyCompanies ?? [];
  const [inputValue, setInputValue] = useState('');

  const add = (raw: string) => {
    // Guard the pre-load window: `jobPrefs` (and thus `list`) is undefined/[]
    // until the query resolves, so an add here would replace the saved list with
    // just this one entry (single-column setter, but still column data loss).
    if (!jobPrefs) return;
    const value = raw.trim();
    if (!value) return;
    if (list.some((c) => c.toLowerCase() === value.toLowerCase())) {
      setInputValue('');
      return;
    }
    setExtraAgencyCompanies.mutate([...list, value]);
    setInputValue('');
  };

  const remove = (name: string) => {
    if (!jobPrefs) return;
    setExtraAgencyCompanies.mutate(list.filter((c) => c !== name));
  };

  return (
    <GlassCard>
      <div className="mb-4 text-xs font-semibold uppercase tracking-[0.16em] text-foreground/55">
        {t('settings.agencyCompanies.title')}
      </div>
      <p className="mb-4 text-sm text-foreground/55">{t('settings.agencyCompanies.description')}</p>

      {list.length > 0 ? (
        <div className="mb-4 flex flex-wrap gap-2">
          <AnimatePresence>
            {list.map((name) => (
              <motion.div
                key={name}
                layout
                initial={{ opacity: 0, scale: 0.9 }}
                animate={{ opacity: 1, scale: 1 }}
                exit={{ opacity: 0, scale: 0.9 }}
                className="flex items-center gap-2 rounded-full bg-foreground/[0.06] px-3 py-1.5 text-sm"
              >
                <Building2 size={14} className="text-foreground/40" />
                <span className="text-foreground">{name}</span>
                <Button
                  variant="unstyled"
                  onClick={() => remove(name)}
                  aria-label={t('settings.agencyCompanies.remove', { name })}
                  className="ml-1 rounded-full p-0.5 transition-colors hover:bg-foreground/10"
                >
                  <X size={12} className="text-foreground/40 hover:text-foreground" />
                </Button>
              </motion.div>
            ))}
          </AnimatePresence>
        </div>
      ) : (
        <p className="mb-4 text-sm text-foreground/40">{t('settings.agencyCompanies.empty')}</p>
      )}

      <div className="flex items-center gap-2">
        <Input
          value={inputValue}
          onChange={(e) => setInputValue(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === 'Enter') {
              e.preventDefault();
              add(inputValue);
            }
          }}
          placeholder={t('settings.agencyCompanies.placeholder')}
          wrapperClassName="flex-1"
        />
        <Button
          variant="glass"
          size="md"
          onClick={() => add(inputValue)}
          disabled={!inputValue.trim()}
        >
          <Plus size={16} /> {t('settings.agencyCompanies.add')}
        </Button>
      </div>
    </GlassCard>
  );
}
