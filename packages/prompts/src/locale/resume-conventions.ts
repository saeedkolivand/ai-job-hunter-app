/**
 * The canonical `SectionId` (`model/document.rs`) Debug names that
 * `locale::resume::section_order_for` actually emits (both `DEFAULT_ORDER`
 * and `DE_ORDER` cover the same 9 ids, just reordered) — NOT the full
 * `SectionId` enum, which also has `Volunteer`/`Interests`/`References`/
 * `Custom`, none of which the résumé section-order prompt ever names.
 *
 * Deliberately the SAME strings Rust's `format!("{id:?}")` produces, so the
 * codegen (`gen-prompts-rust.ts`) can key a Rust lookup off these entries
 * directly instead of maintaining a second TS-name -> Rust-variant mapping
 * table that could silently miss an id.
 */
export type ResumeSectionHeaderId =
  | 'Summary'
  | 'Experience'
  | 'Education'
  | 'Skills'
  | 'Projects'
  | 'Certifications'
  | 'Languages'
  | 'Awards'
  | 'Publications';

export interface ResumeConventions {
  /**
   * Localized section header per ordered id — a TOTAL record, so a new id
   * added to `section_order_for` fails this file's typecheck in every locale
   * below until it is given a real name. Previously only 4 of the 9 ordered
   * ids were covered; the other 5 fell through to the raw English `SectionId`
   * debug word even while the model was told to write, say, German — see
   * `pipeline::resume::prompts::section_order_prompt_list`'s doc comment.
   */
  headers: Record<ResumeSectionHeaderId, string>;
  /** Example of a market-conventional date range. */
  dateExample: string;
}

const EN_CONVENTIONS: ResumeConventions = {
  headers: {
    Summary: 'Professional Summary',
    Experience: 'Work Experience',
    Education: 'Education',
    Skills: 'Skills',
    Projects: 'Projects',
    Certifications: 'Certifications',
    Languages: 'Languages',
    Awards: 'Awards',
    Publications: 'Publications',
  },
  dateExample: 'January 2021 – March 2023',
};

const CONVENTIONS: Record<string, ResumeConventions> = {
  en: EN_CONVENTIONS,
  de: {
    headers: {
      Summary: 'Profil',
      Experience: 'Berufserfahrung',
      Education: 'Ausbildung',
      Skills: 'Kenntnisse',
      Projects: 'Projekte',
      // NOT "Weiterbildung" — it contains "bildung", so
      // `documents::evidence::classify_section` buckets it as Education,
      // colliding with "Ausbildung". "Zertifikate" classifies as `Other`
      // and collides with nothing.
      Certifications: 'Zertifikate',
      // NOT "Sprachkenntnisse" — it contains "kenntnis", so the classifier
      // above buckets it as Skills, not Languages. Plain "Sprachen"
      // classifies as `Other`.
      Languages: 'Sprachen',
      Awards: 'Auszeichnungen',
      Publications: 'Publikationen',
    },
    dateExample: '01/2021 – 03/2023',
  },
  fr: {
    headers: {
      Summary: 'Profil',
      Experience: 'Expérience professionnelle',
      Education: 'Formation',
      Skills: 'Compétences',
      Projects: 'Projets',
      Certifications: 'Certifications',
      Languages: 'Langues',
      // NOT "Prix" — an ordinary noun, and the recogniser side has no shape
      // guard behind it.
      Awards: 'Distinctions',
      Publications: 'Publications',
    },
    dateExample: 'janvier 2021 – mars 2023',
  },
  es: {
    headers: {
      Summary: 'Perfil',
      Experience: 'Experiencia profesional',
      Education: 'Formación',
      Skills: 'Habilidades',
      Projects: 'Proyectos',
      Certifications: 'Certificaciones',
      Languages: 'Idiomas',
      Awards: 'Premios',
      Publications: 'Publicaciones',
    },
    dateExample: 'enero 2021 – marzo 2023',
  },
  it: {
    headers: {
      Summary: 'Profilo',
      Experience: 'Esperienza professionale',
      Education: 'Formazione',
      Skills: 'Competenze',
      Projects: 'Progetti',
      Certifications: 'Certificazioni',
      Languages: 'Lingue',
      Awards: 'Riconoscimenti',
      Publications: 'Pubblicazioni',
    },
    dateExample: 'gennaio 2021 – marzo 2023',
  },
  nl: {
    headers: {
      Summary: 'Profiel',
      Experience: 'Werkervaring',
      Education: 'Opleiding',
      Skills: 'Vaardigheden',
      Projects: 'Projecten',
      Certifications: 'Certificaten',
      Languages: 'Talen',
      Awards: 'Onderscheidingen',
      Publications: 'Publicaties',
    },
    dateExample: '01/2021 – 03/2023',
  },
  pt: {
    headers: {
      Summary: 'Perfil',
      Experience: 'Experiência profissional',
      Education: 'Formação',
      Skills: 'Competências',
      Projects: 'Projetos',
      Certifications: 'Certificações',
      Languages: 'Idiomas',
      // pt-PT spelling ("Prêmios" is pt-BR); nothing in this row
      // discriminates the two — the recogniser side accepts both.
      Awards: 'Prémios',
      Publications: 'Publicações',
    },
    dateExample: 'janeiro 2021 – março 2023',
  },
};

/**
 * Resume conventions for a locale, falling back to English headers + a note to
 * use the market's own conventions when the locale is unknown.
 */
export function resumeConventions(locale?: string): ResumeConventions {
  const key = (locale ?? 'en').slice(0, 2).toLowerCase();
  return CONVENTIONS[key] ?? EN_CONVENTIONS;
}

/** True when we have explicit conventions for the locale (vs. the en fallback). */
export function hasResumeConventions(locale?: string): boolean {
  const key = (locale ?? '').slice(0, 2).toLowerCase();
  return key in CONVENTIONS;
}

/**
 * Every locale {@link resumeConventions} has a curated entry for. Exported so
 * the Rust codegen (`pnpm gen:prompts`) can enumerate the SAME set instead of
 * hardcoding a second list that silently misses a locale added here.
 */
export const RESUME_CONVENTION_LOCALES: readonly string[] = Object.keys(CONVENTIONS);
