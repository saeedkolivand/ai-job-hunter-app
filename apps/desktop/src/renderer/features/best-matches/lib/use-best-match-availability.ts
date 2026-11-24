import { useCallback, useRef, useState } from 'react';

import type { AutopilotAvailabilityCheck, AutopilotBestMatch } from '@ajh/shared';
import { useTranslation } from '@ajh/translations';
import { useNotification } from '@ajh/ui';

import { useCheckAvailability } from '@/services';

const isDead = (c: AutopilotAvailabilityCheck) =>
  c.availability === 'closed' || c.availability === 'removed';

/**
 * Availability checks for the Best Matches page (#1444): the on-demand check, and the check that
 * runs when the user starts Prepare. Verdicts are keyed by the row's stable `key` captured at
 * click time, so a re-sort or refill can never attach one to another job.
 *
 * A dead verdict never blocks silently: Prepare pauses on a warning the user can override. An
 * `unknown` verdict only says so — a failed or blocked check is never treated as closed.
 */
export function useBestMatchAvailability(prepare: (match: AutopilotBestMatch) => void) {
  const { t } = useTranslation();
  const notify = useNotification();
  const { mutateAsync } = useCheckAvailability();
  const [results, setResults] = useState<Record<string, AutopilotAvailabilityCheck>>({});
  const [checking, setChecking] = useState<Set<string>>(new Set());
  const [warning, setWarning] = useState<AutopilotBestMatch | null>(null);
  const prepareRequest = useRef(0);

  const runCheck = useCallback(
    async (match: AutopilotBestMatch) => {
      setChecking((prev) => new Set(prev).add(match.key));
      try {
        const check = await mutateAsync({ url: match.url });
        setResults((prev) => ({ ...prev, [match.key]: check }));
        return check;
      } catch {
        return null;
      } finally {
        setChecking((prev) => {
          const next = new Set(prev);
          next.delete(match.key);
          return next;
        });
      }
    },
    [mutateAsync]
  );

  const handleCheck = useCallback(
    async (match: AutopilotBestMatch) => {
      const check = await runCheck(match);
      if (!check || check.availability === 'unknown') {
        notify.info({ message: t('bestMatches.availability.unverified') });
      }
    },
    [runCheck, notify, t]
  );

  const handlePrepare = useCallback(
    async (match: AutopilotBestMatch) => {
      // The latest Prepare owns the flow: a superseded request's late result is dropped.
      const id = ++prepareRequest.current;
      setWarning(null);
      const check = await runCheck(match);
      if (id !== prepareRequest.current) return;
      if (check && isDead(check)) {
        setWarning(match);
        return;
      }
      if (!check || check.availability === 'unknown') {
        notify.info({ message: t('bestMatches.availability.unverified') });
      }
      prepare(match);
    },
    [runCheck, notify, t, prepare]
  );

  const confirmPrepareAnyway = useCallback(() => {
    prepareRequest.current++;
    if (warning) prepare(warning);
    setWarning(null);
  }, [warning, prepare]);

  const dismissWarning = useCallback(() => {
    prepareRequest.current++;
    setWarning(null);
  }, []);

  /** The row with any verdict from this visit applied over the snapshot's value. */
  const withVerdict = useCallback(
    (match: AutopilotBestMatch): AutopilotBestMatch => {
      const check = results[match.key];
      return check
        ? {
            ...match,
            availability: check.availability,
            availabilityCheckedAt: check.checkedAt,
            availabilitySource: check.source,
          }
        : match;
    },
    [results]
  );

  const isUnavailable = useCallback(
    (key: string) => {
      const check = results[key];
      return check ? isDead(check) : false;
    },
    [results]
  );

  return {
    checking,
    warning,
    dismissWarning,
    confirmPrepareAnyway,
    handleCheck,
    handlePrepare,
    withVerdict,
    isUnavailable,
  };
}
