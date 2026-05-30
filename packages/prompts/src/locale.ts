/**
 * Locale data — section-header lexicons, resume conventions, and token factors.
 *
 * All market/locale behaviour keys off the JOB-AD's detected locale (there is no
 * default-to-German or default-to-English market assumption). Pure data + helpers,
 * no dependencies.
 */

export type SectionName =
  | 'Summary'
  | 'Experience'
  | 'Education'
  | 'Skills'
  | 'Certifications'
  | 'Projects'
  | 'Publications'
  | 'Awards'
  | 'Languages'
  | 'Volunteer'
  | 'Interests';

export interface SectionLexiconEntry {
  name: SectionName;
  priority: number;
  /** Lowercased header terms across supported locales (matched at line start). */
  terms: string[];
}

/**
 * Section-header lexicon spanning the common job-market locales (en, de, fr, es,
 * it, nl, pt). Ordered so the most specific / highest-value sections match first.
 * `detectSections` matches a line against these before falling back to a
 * structural heuristic, so a non-English resume isn't collapsed into one blob.
 */
export const SECTION_LEXICON: SectionLexiconEntry[] = [
  {
    name: 'Summary',
    priority: 9,
    terms: [
      'professional summary',
      'summary',
      'profile',
      'objective',
      'about me',
      'profil',
      'zusammenfassung',
      'kurzprofil',
      'über mich',
      'profil professionnel',
      'résumé',
      'à propos',
      'objectif',
      'perfil',
      'resumen',
      'sobre mí',
      'objetivo',
      'profilo',
      'sommario',
      'riassunto',
      'obiettivo',
      'profiel',
      'samenvatting',
      'over mij',
      'doelstelling',
      'resumo',
      'sobre mim',
    ],
  },
  {
    name: 'Experience',
    priority: 10,
    terms: [
      'work experience',
      'professional experience',
      'experience',
      'employment history',
      'career',
      'berufserfahrung',
      'beruflicher werdegang',
      'werdegang',
      'arbeitserfahrung',
      'praxiserfahrung',
      'expérience professionnelle',
      'expériences professionnelles',
      'expérience',
      'expériences',
      'parcours professionnel',
      'experiencia laboral',
      'experiencia profesional',
      'experiencia',
      'esperienza professionale',
      'esperienza lavorativa',
      'esperienza',
      'esperienze',
      'werkervaring',
      'ervaring',
      'loopbaan',
      'experiência profissional',
      'experiência',
    ],
  },
  {
    name: 'Education',
    priority: 8,
    terms: [
      'education',
      'academic background',
      'qualifications',
      'ausbildung',
      'bildung',
      'studium',
      'schulbildung',
      'akademischer werdegang',
      'formation',
      'éducation',
      'études',
      'diplômes',
      'educación',
      'formación',
      'formación académica',
      'estudios',
      'istruzione',
      'formazione',
      'educazione',
      'studi',
      'opleiding',
      'onderwijs',
      'studie',
      'educação',
      'formação',
      'formação acadêmica',
      'escolaridade',
    ],
  },
  {
    name: 'Skills',
    priority: 9,
    terms: [
      'skills',
      'technical skills',
      'core competencies',
      'expertise',
      'kenntnisse',
      'fähigkeiten',
      'kompetenzen',
      'fachkenntnisse',
      'compétences',
      'savoir-faire',
      'habilidades',
      'competencias',
      'conocimientos',
      'aptitudes',
      'competenze',
      'abilità',
      'conoscenze',
      'vaardigheden',
      'competenties',
      'kennis',
      'competências',
      'habilidades técnicas',
      'conhecimentos',
    ],
  },
  {
    name: 'Certifications',
    priority: 7,
    terms: [
      'certifications',
      'certificates',
      'licenses',
      'zertifikate',
      'zertifizierungen',
      'lizenzen',
      'certifications',
      'certificats',
      'certificaciones',
      'certificados',
      'licencias',
      'certificazioni',
      'certificati',
      'certificeringen',
      'certificaten',
      'certificações',
    ],
  },
  {
    name: 'Projects',
    priority: 6,
    terms: [
      'projects',
      'portfolio',
      'key projects',
      'projekte',
      'projektübersicht',
      'projets',
      'proyectos',
      'progetti',
      'projecten',
      'projetos',
    ],
  },
  {
    name: 'Publications',
    priority: 5,
    terms: [
      'publications',
      'research',
      'papers',
      'publikationen',
      'veröffentlichungen',
      'forschung',
      'recherche',
      'publicaciones',
      'investigación',
      'pubblicazioni',
      'ricerca',
      'publicaties',
      'onderzoek',
      'publicações',
      'pesquisa',
    ],
  },
  {
    name: 'Awards',
    priority: 5,
    terms: [
      'awards',
      'honors',
      'achievements',
      'auszeichnungen',
      'ehrungen',
      'erfolge',
      'prix',
      'distinctions',
      'réalisations',
      'premios',
      'reconocimientos',
      'logros',
      'premi',
      'riconoscimenti',
      'prijzen',
      'onderscheidingen',
      'prêmios',
      'conquistas',
    ],
  },
  {
    name: 'Languages',
    priority: 4,
    terms: [
      'languages',
      'language skills',
      'sprachen',
      'sprachkenntnisse',
      'langues',
      'idiomas',
      'lenguas',
      'lingue',
      'talen',
      'línguas',
    ],
  },
  {
    name: 'Volunteer',
    priority: 3,
    terms: [
      'volunteer',
      'volunteering',
      'community',
      'ehrenamt',
      'freiwilligenarbeit',
      'bénévolat',
      'volontariat',
      'voluntariado',
      'volontariato',
      'vrijwilligerswerk',
    ],
  },
  {
    name: 'Interests',
    priority: 2,
    terms: [
      'interests',
      'hobbies',
      'interessen',
      'hobbys',
      "centres d'intérêt",
      'loisirs',
      'intérêts',
      'intereses',
      'aficiones',
      'pasatiempos',
      'interessi',
      'hobby',
      'interesses',
      'passatempos',
    ],
  },
];

// ─── Resume conventions per locale ────────────────────────────────────────────

export interface ResumeConventions {
  /** Localized standard section headers the output should use. */
  headers: { summary: string; experience: string; education: string; skills: string };
  /** Example of a market-conventional date range. */
  dateExample: string;
}

const EN_CONVENTIONS: ResumeConventions = {
  headers: {
    summary: 'Professional Summary',
    experience: 'Work Experience',
    education: 'Education',
    skills: 'Skills',
  },
  dateExample: 'January 2021 – March 2023',
};

const CONVENTIONS: Record<string, ResumeConventions> = {
  en: EN_CONVENTIONS,
  de: {
    headers: {
      summary: 'Profil',
      experience: 'Berufserfahrung',
      education: 'Ausbildung',
      skills: 'Kenntnisse',
    },
    dateExample: '01/2021 – 03/2023',
  },
  fr: {
    headers: {
      summary: 'Profil',
      experience: 'Expérience professionnelle',
      education: 'Formation',
      skills: 'Compétences',
    },
    dateExample: 'janvier 2021 – mars 2023',
  },
  es: {
    headers: {
      summary: 'Perfil',
      experience: 'Experiencia profesional',
      education: 'Formación',
      skills: 'Habilidades',
    },
    dateExample: 'enero 2021 – marzo 2023',
  },
  it: {
    headers: {
      summary: 'Profilo',
      experience: 'Esperienza professionale',
      education: 'Formazione',
      skills: 'Competenze',
    },
    dateExample: 'gennaio 2021 – marzo 2023',
  },
  nl: {
    headers: {
      summary: 'Profiel',
      experience: 'Werkervaring',
      education: 'Opleiding',
      skills: 'Vaardigheden',
    },
    dateExample: '01/2021 – 03/2023',
  },
  pt: {
    headers: {
      summary: 'Perfil',
      experience: 'Experiência profissional',
      education: 'Formação',
      skills: 'Competências',
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

// ─── Token estimation factors ─────────────────────────────────────────────────

/**
 * Characters-per-token by locale. `length / 4` (English) under-counts tokens for
 * languages that tokenizers split more aggressively (German, Dutch, …), so those
 * use a smaller divisor → a higher token estimate.
 */
export const CHARS_PER_TOKEN: Record<string, number> = {
  en: 4,
  de: 3.2,
  nl: 3.4,
  fr: 3.6,
  es: 3.7,
  it: 3.7,
  pt: 3.7,
};

/** Characters-per-token divisor for a locale (default 4). */
export function charsPerToken(locale?: string): number {
  const key = (locale ?? 'en').slice(0, 2).toLowerCase();
  return CHARS_PER_TOKEN[key] ?? 4;
}
