import { createContext } from 'react';

/**
 * Shared by {@link Image.PreviewGroup} and its child {@link Image}s. Children
 * register their src on mount (receiving a stable id) and call `onPreview(id)` on
 * click; the group owns a single lightbox and opens it at the clicked image with
 * prev/next navigation across all registered srcs.
 */
export interface PreviewGroupContextValue {
  /** Register a previewable src; returns a stable id for later `onPreview`. */
  register: (src: string) => number;
  /** Drop a registration on unmount. */
  unregister: (id: number) => void;
  /** Open the group lightbox focused on the image with this id. */
  onPreview: (id: number) => void;
}

export const PreviewGroupContext = createContext<PreviewGroupContextValue | null>(null);
