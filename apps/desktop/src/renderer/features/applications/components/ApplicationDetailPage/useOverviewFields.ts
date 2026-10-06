import { useState } from 'react';

import type { Application } from '@ajh/shared';

import { useSyncedBuffer } from '@/features/applications/lib/use-synced-buffer';
import { useUpdateApplication } from '@/services';

import { fromDateInputValue, toDateInputValue } from './detail-format';

type UpdateApplication = ReturnType<typeof useUpdateApplication>;

/** A save-on-blur text field: buffer + commit-on-blur, no error surface. */
function useBufferedField(
  serverValue: string,
  commit: (value: string) => void
): { value: string; onChange: (v: string) => void; onBlur: () => void } {
  const [value, setValue] = useSyncedBuffer(serverValue);
  return {
    value,
    onChange: setValue,
    onBlur: () => {
      if (value !== serverValue) commit(value);
    },
  };
}

/**
 * A contact field whose write the backend may reject (e.g. a malformed email).
 * A rejected write returns `{ error }` rather than throwing — surface it here
 * exactly as ApplyByEmailTab does for the same canonical pair.
 */
function useContactField(
  serverValue: string,
  update: UpdateApplication,
  patch: (value: string) => Parameters<UpdateApplication['mutate']>[0]
) {
  const [error, setError] = useState(false);
  const field = useBufferedField(serverValue, (value) =>
    update.mutate(patch(value), {
      onSuccess: (data) => setError(!!data.error),
      onError: () => setError(true),
    })
  );
  return {
    ...field,
    error,
    onChange: (v: string) => {
      field.onChange(v);
      setError(false);
    },
  };
}

/**
 * Save-on-blur editable buffers for the Overview tab. They live in the page-level
 * loaded view (not the tab) so each re-seeds independently when ITS server value
 * changes (see `useSyncedBuffer`) — no remount, so focus and sibling uncommitted
 * text survive a write landing, and a tab switch does not drop them.
 */
export function useOverviewFields(application: Application) {
  const update = useUpdateApplication();
  const { id } = application;

  const notes = useBufferedField(application.notes, (value) => update.mutate({ id, notes: value }));
  const comp = useBufferedField(application.comp, (value) => update.mutate({ id, comp: value }));
  const contactName = useContactField(application.contactName, update, (contactName) => ({
    id,
    contactName,
  }));
  const contactEmail = useContactField(application.contactEmail, update, (contactEmail) => ({
    id,
    contactEmail,
  }));

  const [nextActionAt, setNextActionAt] = useSyncedBuffer(
    toDateInputValue(application.nextActionAt)
  );
  const nextAction = {
    value: nextActionAt,
    onChange: setNextActionAt,
    onBlur: () => {
      const next = fromDateInputValue(nextActionAt);
      if (next !== (application.nextActionAt ?? null)) {
        update.mutate({ id, nextActionAt: next });
      }
    },
  };

  return { nextAction, notes, contactName, contactEmail, comp };
}

export type OverviewFields = ReturnType<typeof useOverviewFields>;
