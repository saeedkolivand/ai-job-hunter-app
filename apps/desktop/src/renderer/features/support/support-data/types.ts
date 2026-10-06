export type Translate = (key: string) => string;

interface Problem {
  /** Stable list key — the leaf of this entry's translation key. */
  id: string;
  q: string;
  a: string;
}

export interface Section {
  /**
   * Stable section id — the `support.faq.<id>` label key, and the prefix of its
   * entries' `support.faq.<id>Questions.*` keys. Not localized, so it is the
   * only thing that can identify a section in code: `label` is user-visible
   * copy that changes with the language.
   */
  id: string;
  icon: React.ElementType;
  label: string;
  color: string;
  glow: string;
  problems: Problem[];
}
