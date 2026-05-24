/**
 * Client-side language detection using franc.
 * Provides accurate language detection for resume and job ad text.
 */

import { franc } from 'franc';

/**
 * ISO 639-1 language code mapping from franc's ISO 639-3 codes.
 */
const LANGUAGE_MAP: Record<string, string> = {
  eng: 'en',
  deu: 'de',
  fra: 'fr',
  spa: 'es',
  ita: 'it',
  por: 'pt',
  rus: 'ru',
  zho: 'zh',
  jpn: 'ja',
  kor: 'ko',
  ara: 'ar',
  hin: 'hi',
  tur: 'tr',
  nld: 'nl',
  pol: 'pl',
  swe: 'swe',
  nor: 'no',
  dan: 'da',
  fin: 'fi',
  ces: 'cs',
  hun: 'hu',
  ron: 'ro',
  ell: 'el',
  heb: 'he',
  tha: 'th',
  vie: 'vi',
  ind: 'id',
};

/**
 * Language name mapping for display purposes.
 */
const LANGUAGE_NAMES: Record<string, string> = {
  en: 'English',
  de: 'German',
  fr: 'French',
  es: 'Spanish',
  it: 'Italian',
  pt: 'Portuguese',
  ru: 'Russian',
  zh: 'Chinese',
  ja: 'Japanese',
  ko: 'Korean',
  ar: 'Arabic',
  hi: 'Hindi',
  tr: 'Turkish',
  nl: 'Dutch',
  pl: 'Polish',
  swe: 'Swedish',
  no: 'Norwegian',
  da: 'Danish',
  fi: 'Finnish',
  cs: 'Czech',
  hu: 'Hungarian',
  ro: 'Romanian',
  el: 'Greek',
  he: 'Hebrew',
  th: 'Thai',
  vi: 'Vietnamese',
  id: 'Indonesian',
};

/**
 * Detect language of text using franc.
 * Returns ISO 639-1 language code (e.g., 'en', 'de').
 * Returns 'unknown' if detection fails or text is too short.
 */
export function detectLanguage(text: string): string {
  if (!text || text.length < 20) {
    return 'unknown';
  }

  const code = franc(text);

  // franc returns 'und' for undetermined
  if (code === 'und') {
    return 'unknown';
  }

  // Map ISO 639-3 to ISO 639-1
  return LANGUAGE_MAP[code] || code;
}

/**
 * Get human-readable language name from ISO 639-1 code.
 */
export function getLanguageName(code: string): string {
  return LANGUAGE_NAMES[code] || code;
}

/**
 * Detect languages for both resume and job ad.
 * Returns detected languages and whether they match.
 */
export function detectLanguages(
  resume: string,
  jobAd: string
): {
  resume: string;
  jobAd: string;
  resumeName: string;
  jobAdName: string;
  mismatch: boolean;
} {
  const resumeLang = detectLanguage(resume);
  const jobAdLang = detectLanguage(jobAd);

  return {
    resume: resumeLang,
    jobAd: jobAdLang,
    resumeName: getLanguageName(resumeLang),
    jobAdName: getLanguageName(jobAdLang),
    mismatch: resumeLang !== 'unknown' && jobAdLang !== 'unknown' && resumeLang !== jobAdLang,
  };
}
