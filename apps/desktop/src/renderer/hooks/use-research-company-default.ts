import { useEffect, useRef, useState } from 'react';

import { shouldSeedResearchDefault } from '@/lib/research-company-default';
import { useActiveModelCapabilities } from '@/services';

/**
 * Local "search company" toggle whose default is capability-driven: ON when the
 * active model can web-search, OFF otherwise. Drop-in for `useState`, returning
 * `[value, setValue]`.
 *
 * The default FOLLOWS the active model: it seeds on the first capability resolve
 * and re-seeds when a mid-session model switch flips the capability — as long as
 * the user hasn't manually toggled it (an explicit choice is sticky and never
 * clobbered). The seed DECISION is the shared {@link shouldSeedResearchDefault}
 * helper; this hook only owns the `useState` + ref plumbing.
 *
 * Flash: `useState` is lazily initialized from the capability query, which uses a
 * long staleTime — so on any load AFTER the first-ever one the value is already
 * cached and there is no ON-flash. The first-EVER (uncached) load starts `false`
 * and flips once the async query resolves; that single frame is inherent to async
 * resolution and not worth heavier machinery to shave.
 */
export function useResearchCompanyDefault(): readonly [boolean, (v: boolean) => void] {
  const { data, isSuccess } = useActiveModelCapabilities();
  const supportsWebSearch = data?.supportsWebSearch ?? false;

  const [value, setValueState] = useState(() => (isSuccess ? supportsWebSearch : false));
  const userTouched = useRef(false);
  const lastSeeded = useRef<boolean | null>(isSuccess ? supportsWebSearch : null);

  useEffect(() => {
    const { seed, value: next } = shouldSeedResearchDefault({
      capabilityResolved: isSuccess,
      supportsWebSearch,
      userTouched: userTouched.current,
      lastSeededValue: lastSeeded.current,
    });
    if (!seed) return;
    lastSeeded.current = next;
    setValueState(next);
  }, [isSuccess, supportsWebSearch]);

  const setValue = (v: boolean) => {
    userTouched.current = true;
    setValueState(v);
  };

  return [value, setValue] as const;
}
