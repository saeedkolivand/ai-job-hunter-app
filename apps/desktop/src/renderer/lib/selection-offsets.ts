/**
 * Selection offsets relative to `container`'s text, or null when nothing inside
 * it is selected. Mirrors the textarea-offset capture in EditableOutput's
 * `openSourceRewrite`, but for a plain (non-input) selectable element.
 *
 * Shared by the select-to-rewrite surfaces (application-answer cards, the
 * apply-by-email draft) so the offset maths lives in one place.
 */
export function getSelectionOffsets(container: HTMLElement): { start: number; end: number } | null {
  const sel = window.getSelection();
  if (!sel || sel.isCollapsed || sel.rangeCount === 0) return null;
  const range = sel.getRangeAt(0);
  if (!container.contains(range.commonAncestorContainer)) return null;
  const pre = document.createRange();
  pre.selectNodeContents(container);
  pre.setEnd(range.startContainer, range.startOffset);
  const start = pre.toString().length;
  return { start, end: start + range.toString().length };
}
