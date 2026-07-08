import { useEffect, useRef, useState } from 'react';

import { useActiveModelCapabilities } from '@/services';

/**
 * Local "search company" toggle whose INITIAL default is capability-driven: ON
 * when the active model can web-search, OFF otherwise. Drop-in for `useState`,
 * returning `[value, setValue]`.
 *
 * Starts `false` (the safe fallback while the capability is still resolving),
 * then flips to the model's `supportsWebSearch` exactly once — and never after
 * the user has toggled it (tracked via a ref) — so an explicit override is never
 * clobbered on re-render or a late resolve. Only the initial default is
 * capability-driven.
 */
export function useResearchCompanyDefault(): readonly [boolean, (v: boolean) => void] {
  const { data, isSuccess } = useActiveModelCapabilities();
  const [value, setValueState] = useState(false);
  const userTouched = useRef(false);
  const applied = useRef(false);

  useEffect(() => {
    if (userTouched.current || applied.current || !isSuccess) return;
    applied.current = true;
    setValueState(data?.supportsWebSearch ?? false);
  }, [isSuccess, data]);

  const setValue = (v: boolean) => {
    userTouched.current = true;
    setValueState(v);
  };

  return [value, setValue] as const;
}
