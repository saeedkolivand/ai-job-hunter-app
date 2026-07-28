import { useState } from 'react';

/**
 * Save-on-blur edit buffer that adopts an out-of-band server change WITHOUT a
 * remount (React's "adjust state during render" pattern).
 *
 * Per FIELD, deliberately: the buffer re-seeds only when the server value for
 * THIS field changed since the previous render, so a write landing for a sibling
 * field (or a status/note write, which also bumps the record) never clobbers
 * text the user is still typing here. Replaces the `key={id}:{updatedAt}`
 * remount, which fixed the same stale-seed wipe but destroyed focus, uncommitted
 * input and the whole TailorFlow sub-tree on every persisted write.
 *
 * Shared: the Overview contact card and the apply-by-email recipient pair edit
 * the SAME canonical `contactName`/`contactEmail` columns from two surfaces, so
 * both must re-seed the same way or whichever one was mounted first persists a
 * stale value back over the other's write.
 */
export function useSyncedBuffer(serverValue: string): [string, (value: string) => void] {
  const [value, setValue] = useState(serverValue);
  const [seen, setSeen] = useState(serverValue);
  if (seen !== serverValue) {
    setSeen(serverValue);
    setValue(serverValue);
  }
  return [value, setValue];
}
