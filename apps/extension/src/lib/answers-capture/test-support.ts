/**
 * Shared DOM helpers for the answers-capture suites: build a real form in the
 * shared jsdom `document`, then run the REAL collectors against it.
 */

export function setForm(html: string): void {
  document.body.innerHTML = `<form>${html}</form>`;
}

/** Remove everything a test put in the document (call from `afterEach`). */
export function resetDocument(): void {
  document.body.innerHTML = '';
  document.head.querySelectorAll('style[data-ajh-test]').forEach((s) => s.remove());
}

/** A labelled input: `<label for=id>label</label><input id=id type=text value=…>`. */
export const field = (id: string, label: string, value = '', attrs = ''): string =>
  `<label for="${id}">${label}</label><input id="${id}" type="text" value="${value}" ${attrs} />`;
