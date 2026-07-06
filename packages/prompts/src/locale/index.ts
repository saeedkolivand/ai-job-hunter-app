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

// ─── Cover-letter market conventions ──────────────────────────────────────────
//
// Per-market letter etiquette + layout. The runtime source of truth is the
// `LETTER_MARKET_CONVENTIONS` const below; the identical data also lives in
// `../fixtures/letter-conventions.json`, which a parity test pins this const to
// and which the Rust renderer mirrors (same pattern as `url-labels.json` ↔
// `url_label`). Keeping the const in code — not a runtime JSON import — avoids a
// dist build that would need the `.json` copied alongside `tsc` output.

export type LetterFormality = 'formal' | 'warm' | 'direct';
export type DatePosition = 'top-right' | 'below-header' | 'above-salutation';
export type SenderPosition = 'top-left' | 'top-right';
export type RecipientPosition = 'left' | 'right' | 'top-right';

/** Cover-letter conventions for one market (country/region). */
export interface LetterMarketConventions {
  /** English country/region name (for UI + the prompt). */
  country: string;
  /** ISO-639-1 of the market's native language. */
  nativeLanguage: string;
  formality: LetterFormality;
  lengthWords: { min: number; max: number };
  /** Physical page — `letter` only for the US; everyone else A4. */
  page: 'a4' | 'letter';
  dateFormat: string;
  datePosition: DatePosition;
  senderPosition: SenderPosition;
  recipientPosition: RecipientPosition;
  /** Whether the market uses a subject line, plus its localized label. */
  subjectLine: { use: boolean; label: string };
  /** Native-language salutations; the prompt translates them to the letter language when it differs. */
  salutations: { named: string; generic: string };
  /** Native-language sign-offs (first = most formal/default). */
  signoffs: string[];
  /** Market-expected content (e.g. DACH → salary expectation + start date). User-supplied only. */
  inclusions: string[];
  notes: string;
}

const DACH_SALUTATIONS = {
  named: 'Sehr geehrte Frau {lastName}, / Sehr geehrter Herr {lastName},',
  generic: 'Sehr geehrte Damen und Herren,',
} as const;

/** The international baseline — also the guaranteed fallback for unknown markets. */
const INTL_LETTER_CONVENTIONS: LetterMarketConventions = {
  country: 'International',
  nativeLanguage: 'en',
  formality: 'warm',
  lengthWords: { min: 200, max: 350 },
  page: 'a4',
  dateFormat: 'D Month YYYY',
  datePosition: 'below-header',
  senderPosition: 'top-left',
  recipientPosition: 'left',
  subjectLine: { use: false, label: '' },
  salutations: { named: 'Dear {title} {lastName},', generic: 'Dear Hiring Manager,' },
  signoffs: ['Sincerely,', 'Kind regards,'],
  inclusions: [],
  notes:
    'Clean, professional international baseline (used when the market is unknown). A4, one page, no special inclusions.',
};

/**
 * Runtime source of truth for letter conventions (16 markets + `intl` fallback).
 * Pinned to `fixtures/letter-conventions.json` by a parity test; the Rust
 * renderer mirrors the same fixture.
 */
export const LETTER_MARKET_CONVENTIONS: Record<string, LetterMarketConventions> = {
  us: {
    country: 'United States',
    nativeLanguage: 'en',
    formality: 'warm',
    lengthWords: { min: 200, max: 350 },
    page: 'letter',
    dateFormat: 'Month D, YYYY',
    datePosition: 'below-header',
    senderPosition: 'top-left',
    recipientPosition: 'left',
    subjectLine: { use: false, label: '' },
    salutations: { named: 'Dear {title} {lastName},', generic: 'Dear Hiring Manager,' },
    signoffs: ['Sincerely,', 'Best regards,'],
    inclusions: [],
    notes:
      'Direct, confident, enthusiastic. Lead with achievements and value. One page. No salary or personal details. Punctuation throughout (comma after salutation).',
  },
  uk: {
    country: 'United Kingdom',
    nativeLanguage: 'en',
    formality: 'formal',
    lengthWords: { min: 200, max: 350 },
    page: 'a4',
    dateFormat: 'D Month YYYY',
    datePosition: 'below-header',
    senderPosition: 'top-right',
    recipientPosition: 'left',
    subjectLine: { use: true, label: 'Re:' },
    salutations: { named: 'Dear Mr/Ms {lastName}', generic: 'Dear Sir or Madam' },
    signoffs: ['Yours sincerely', 'Yours faithfully'],
    inclusions: [],
    notes:
      "Polite professionalism. NO punctuation in the salutation, sign-off, date or address. 'Yours sincerely' when the name is known, 'Yours faithfully' when addressed to Sir/Madam. One page.",
  },
  de: {
    country: 'Germany',
    nativeLanguage: 'de',
    formality: 'formal',
    lengthWords: { min: 250, max: 400 },
    page: 'a4',
    dateFormat: 'D. Month YYYY',
    datePosition: 'top-right',
    senderPosition: 'top-right',
    recipientPosition: 'left',
    subjectLine: { use: true, label: 'Betreff' },
    salutations: { ...DACH_SALUTATIONS },
    signoffs: ['Mit freundlichen Grüßen'],
    inclusions: [
      'salary expectation (Gehaltsvorstellung)',
      'earliest possible start date (Eintrittstermin)',
    ],
    notes:
      'DIN 5008. Formal even in creative industries. Always use the named contact when given. A bold subject line (Betreff) before the salutation. State salary expectation and earliest start date only if the applicant supplied them. One page.',
  },
  at: {
    country: 'Austria',
    nativeLanguage: 'de',
    formality: 'formal',
    lengthWords: { min: 250, max: 400 },
    page: 'a4',
    dateFormat: 'D. Month YYYY',
    datePosition: 'top-right',
    senderPosition: 'top-right',
    recipientPosition: 'left',
    subjectLine: { use: true, label: 'Betreff' },
    salutations: { ...DACH_SALUTATIONS },
    signoffs: ['Mit freundlichen Grüßen'],
    inclusions: [
      'salary expectation (Gehaltsvorstellung)',
      'earliest possible start date (Eintrittstermin)',
    ],
    notes:
      'Like Germany (DIN 5008): formal, named contact, bold Betreff, salary + start date when supplied. One page.',
  },
  ch: {
    country: 'Switzerland',
    nativeLanguage: 'de',
    formality: 'formal',
    lengthWords: { min: 250, max: 400 },
    page: 'a4',
    dateFormat: 'D. Month YYYY',
    datePosition: 'top-right',
    senderPosition: 'top-right',
    recipientPosition: 'left',
    subjectLine: { use: true, label: 'Betreff' },
    salutations: { ...DACH_SALUTATIONS },
    signoffs: ['Mit freundlichen Grüssen'],
    inclusions: ['earliest possible start date (Eintrittstermin)'],
    notes: "Swiss German uses 'Grüssen' (ss, no ß). Formal, named contact, bold Betreff. One page.",
  },
  fr: {
    country: 'France',
    nativeLanguage: 'fr',
    formality: 'formal',
    lengthWords: { min: 180, max: 320 },
    page: 'a4',
    dateFormat: 'le D Month YYYY',
    datePosition: 'above-salutation',
    senderPosition: 'top-left',
    recipientPosition: 'top-right',
    subjectLine: { use: true, label: 'Objet' },
    salutations: { named: 'Madame, / Monsieur,', generic: 'Madame, Monsieur,' },
    signoffs: [
      "Je vous prie d'agréer, Madame, Monsieur, l'expression de mes salutations distinguées.",
      'Cordialement,',
    ],
    inclusions: [],
    notes:
      "Very formal, motivation-focused. Sender top-left, employer top-right, city + date above an 'Objet' line. Half-to-one page; never exceed one page. Mirror the salutation gender/number in the closing formula.",
  },
  es: {
    country: 'Spain',
    nativeLanguage: 'es',
    formality: 'formal',
    lengthWords: { min: 220, max: 450 },
    page: 'a4',
    dateFormat: 'D de Month de YYYY',
    datePosition: 'below-header',
    senderPosition: 'top-left',
    recipientPosition: 'left',
    subjectLine: { use: false, label: 'Asunto' },
    salutations: { named: 'Estimado/a Sr./Sra. {lastName}:', generic: 'Estimado/a Sr./Sra.:' },
    signoffs: ['Atentamente,', 'Saludos cordiales,'],
    inclusions: [],
    notes:
      'Formal. The salutation ends with a COLON, not a comma. May run a little longer / more detailed than a US letter, but stay focused.',
  },
  it: {
    country: 'Italy',
    nativeLanguage: 'it',
    formality: 'formal',
    lengthWords: { min: 200, max: 350 },
    page: 'a4',
    dateFormat: 'D Month YYYY',
    datePosition: 'below-header',
    senderPosition: 'top-left',
    recipientPosition: 'left',
    subjectLine: { use: true, label: 'Oggetto' },
    salutations: { named: 'Gentile {title} {lastName},', generic: 'Spettabile Azienda,' },
    signoffs: ['Distinti saluti,', 'Cordiali saluti,'],
    inclusions: [],
    notes:
      "Formal and slightly deferential. 'Egregio/Gentile' + title for a known recipient, 'Spettabile' for a company. One page.",
  },
  pt: {
    country: 'Portugal',
    nativeLanguage: 'pt',
    formality: 'formal',
    lengthWords: { min: 200, max: 350 },
    page: 'a4',
    dateFormat: 'D de Month de YYYY',
    datePosition: 'below-header',
    senderPosition: 'top-left',
    recipientPosition: 'left',
    subjectLine: { use: true, label: 'Assunto' },
    salutations: { named: 'Exmo.(a) Senhor(a) {lastName},', generic: 'Exmos. Senhores,' },
    signoffs: ['Com os melhores cumprimentos,', 'Atenciosamente,'],
    inclusions: [],
    notes: 'Formal European Portuguese. One page.',
  },
  br: {
    country: 'Brazil',
    nativeLanguage: 'pt',
    formality: 'formal',
    lengthWords: { min: 200, max: 350 },
    page: 'a4',
    dateFormat: 'D de Month de YYYY',
    datePosition: 'below-header',
    senderPosition: 'top-left',
    recipientPosition: 'left',
    subjectLine: { use: false, label: 'Assunto' },
    salutations: {
      named: 'Prezado(a) Sr./Sra. {lastName},',
      generic: 'Prezados(as) Senhores(as),',
    },
    signoffs: ['Atenciosamente,', 'Cordialmente,'],
    inclusions: [],
    notes: 'Brazilian Portuguese — slightly warmer than Portugal, still professional. One page.',
  },
  tr: {
    country: 'Turkey',
    nativeLanguage: 'tr',
    formality: 'formal',
    lengthWords: { min: 200, max: 350 },
    page: 'a4',
    dateFormat: 'D Month YYYY',
    datePosition: 'below-header',
    senderPosition: 'top-left',
    recipientPosition: 'left',
    subjectLine: { use: true, label: 'Konu' },
    salutations: { named: 'Sayın {lastName} Bey/Hanım,', generic: 'Sayın Yetkili,' },
    signoffs: ['Saygılarımla,', 'İyi çalışmalar dilerim,'],
    inclusions: [],
    notes:
      'Highly formal and polite; address by title (Bey/Hanım). Indirect, respectful tone. One page.',
  },
  ru: {
    country: 'Russia',
    nativeLanguage: 'ru',
    formality: 'formal',
    lengthWords: { min: 150, max: 300 },
    page: 'a4',
    dateFormat: 'D Month YYYY',
    datePosition: 'below-header',
    senderPosition: 'top-left',
    recipientPosition: 'left',
    subjectLine: { use: false, label: 'Тема' },
    salutations: { named: 'Уважаемый(ая) {firstName} {patronymic}!', generic: 'Здравствуйте!' },
    signoffs: ['С уважением,'],
    inclusions: [],
    notes:
      "Short, factual, formal and hierarchical. Sender block (name/address/phone/email) top-left, then date, then recipient. Don't over-detail. One page.",
  },
  cn: {
    country: 'China',
    nativeLanguage: 'zh',
    formality: 'formal',
    lengthWords: { min: 180, max: 320 },
    page: 'a4',
    dateFormat: 'YYYY年M月D日',
    datePosition: 'below-header',
    senderPosition: 'top-left',
    recipientPosition: 'left',
    subjectLine: { use: false, label: '主题' },
    salutations: { named: '尊敬的{lastName}{title}：', generic: '尊敬的招聘经理：' },
    signoffs: ['此致 敬礼', 'Sincerely,'],
    inclusions: [],
    notes:
      'Respectful and formal. Name + contact at the top. 3–4 short paragraphs, one page. (Domestic Chinese applications rarely include a cover letter; this targets international/foreign employers.)',
  },
  jp: {
    country: 'Japan',
    nativeLanguage: 'ja',
    formality: 'formal',
    lengthWords: { min: 180, max: 320 },
    page: 'a4',
    dateFormat: 'YYYY年M月D日',
    datePosition: 'below-header',
    senderPosition: 'top-right',
    recipientPosition: 'left',
    subjectLine: { use: false, label: '件名' },
    salutations: { named: '採用ご担当 {lastName} 様', generic: '採用ご担当者様' },
    signoffs: ['よろしくお願い申し上げます。', '敬具'],
    inclusions: [],
    notes:
      'Very formal and humble; never oversell. Concise, 3–4 short paragraphs, one page. Express respect and appreciation. In a non-Japanese language, keep the same formal, modest register.',
  },
  kr: {
    country: 'South Korea',
    nativeLanguage: 'ko',
    formality: 'formal',
    lengthWords: { min: 200, max: 380 },
    page: 'a4',
    dateFormat: 'YYYY년 M월 D일',
    datePosition: 'below-header',
    senderPosition: 'top-left',
    recipientPosition: 'left',
    subjectLine: { use: false, label: '제목' },
    salutations: { named: '{lastName}님께,', generic: '채용 담당자님께,' },
    signoffs: ['감사합니다.'],
    inclusions: [],
    notes:
      'Formal and humble (self-introduction style); overselling reads as arrogant. Polite salutation and closing, sincere motivation. One page.',
  },
  intl: INTL_LETTER_CONVENTIONS,
};

/** All known market ids (for UI pickers + tests). */
export const LETTER_MARKET_IDS = Object.keys(LETTER_MARKET_CONVENTIONS);

/** Letter conventions for a market id, falling back to the international baseline. */
export function letterConventions(market?: string): LetterMarketConventions {
  const key = (market ?? 'intl').trim().toLowerCase();
  return LETTER_MARKET_CONVENTIONS[key] ?? INTL_LETTER_CONVENTIONS;
}

/** True when we have explicit conventions for the market (vs. the intl fallback). */
export function hasLetterConventions(market?: string): boolean {
  const key = (market ?? '').trim().toLowerCase();
  return key in LETTER_MARKET_CONVENTIONS;
}

/**
 * ISO-3166 alpha-2 country → market id. Country splits that matter (US vs UK,
 * DE/AT/CH) are explicit; English-speaking peers map to the closest convention
 * set, and most Spanish-/Portuguese-speaking countries share es/pt.
 */
const COUNTRY_TO_MARKET: Record<string, string> = {
  US: 'us',
  GB: 'uk',
  UK: 'uk',
  IE: 'uk',
  AU: 'uk',
  NZ: 'uk',
  CA: 'us',
  IN: 'uk',
  SG: 'uk',
  DE: 'de',
  AT: 'at',
  CH: 'ch',
  FR: 'fr',
  BE: 'fr',
  LU: 'fr',
  MC: 'fr',
  ES: 'es',
  MX: 'es',
  AR: 'es',
  CL: 'es',
  CO: 'es',
  PE: 'es',
  IT: 'it',
  PT: 'pt',
  BR: 'br',
  TR: 'tr',
  RU: 'ru',
  CN: 'cn',
  TW: 'cn',
  HK: 'cn',
  JP: 'jp',
  KR: 'kr',
};

/** Letter language (ISO-639-1) → default market when no country is known. */
const LANGUAGE_TO_MARKET: Record<string, string> = {
  en: 'intl',
  de: 'de',
  fr: 'fr',
  es: 'es',
  it: 'it',
  pt: 'pt',
  tr: 'tr',
  ru: 'ru',
  zh: 'cn',
  ja: 'jp',
  ko: 'kr',
  nl: 'intl',
};

/** Map an ISO-3166 alpha-2 country code to a market id (undefined when unknown). */
export function countryToMarket(country?: string): string | undefined {
  if (!country) return undefined;
  return COUNTRY_TO_MARKET[country.trim().toUpperCase()];
}

/**
 * ISO-3166 alpha-2 country → ISO-4217 currency code. Wider than
 * {@link COUNTRY_TO_MARKET} (also covers Nordics/CEE/Eurozone countries with
 * no distinct letter-conventions entry) — grounds the web-researched salary
 * range (see `salary_research` on the Rust side) in the job's actual currency
 * so a blank/weak location can't let the model default to USD or hallucinate
 * one.
 *
 * ponytail: kept as its own map, not derived from {@link COUNTRY_TO_MARKET} —
 * the two group countries differently (e.g. all Eurozone members share one
 * currency but split across `de`/`fr`/`es`/`it` letter-convention markets), so
 * a derived map would need an inverse lookup with no real gain.
 */
const COUNTRY_TO_CURRENCY: Record<string, string> = {
  US: 'USD',
  GB: 'GBP',
  UK: 'GBP',
  IE: 'EUR',
  AU: 'AUD',
  NZ: 'NZD',
  CA: 'CAD',
  IN: 'INR',
  SG: 'SGD',
  DE: 'EUR',
  AT: 'EUR',
  CH: 'CHF',
  FR: 'EUR',
  BE: 'EUR',
  LU: 'EUR',
  MC: 'EUR',
  ES: 'EUR',
  MX: 'MXN',
  AR: 'ARS',
  CL: 'CLP',
  CO: 'COP',
  PE: 'PEN',
  IT: 'EUR',
  PT: 'EUR',
  BR: 'BRL',
  TR: 'TRY',
  RU: 'RUB',
  CN: 'CNY',
  TW: 'TWD',
  HK: 'HKD',
  JP: 'JPY',
  KR: 'KRW',
  SE: 'SEK',
  NO: 'NOK',
  DK: 'DKK',
  PL: 'PLN',
  CZ: 'CZK',
  SK: 'EUR',
  SI: 'EUR',
  EE: 'EUR',
  LV: 'EUR',
  LT: 'EUR',
  CY: 'EUR',
  MT: 'EUR',
  GR: 'EUR',
  FI: 'EUR',
  NL: 'EUR',
  HR: 'EUR',
  BG: 'EUR',
  HU: 'HUF',
  RO: 'RON',
  SM: 'EUR',
  VA: 'EUR',
  AD: 'EUR',
};

/** Map an ISO-3166 alpha-2 country code to its ISO-4217 currency (undefined when unknown). */
export function countryToCurrency(country?: string): string | undefined {
  if (!country) return undefined;
  return COUNTRY_TO_CURRENCY[country.trim().toUpperCase()];
}

export interface ResolveMarketInput {
  /** ISO-3166 alpha-2 country extracted from the job ad. */
  jobCountry?: string;
  /** Country inferred from the company-research brief HQ (fallback when the ad is silent). */
  briefCountry?: string;
  /** Letter target language (BCP-47 / ISO-639-1). */
  targetLanguage?: string;
  /** Explicit user-chosen market id (highest priority). */
  override?: string;
}

/**
 * Resolve the cover-letter market id. Priority: explicit override → job country
 * → research-brief HQ country → letter-language default → international. Always
 * returns a valid id that {@link letterConventions} can resolve.
 */
export function resolveMarket(input: ResolveMarketInput): string {
  const { jobCountry, briefCountry, targetLanguage, override } = input;
  if (override && hasLetterConventions(override)) return override.trim().toLowerCase();
  return (
    countryToMarket(jobCountry) ??
    countryToMarket(briefCountry) ??
    LANGUAGE_TO_MARKET[(targetLanguage ?? '').slice(0, 2).toLowerCase()] ??
    'intl'
  );
}
