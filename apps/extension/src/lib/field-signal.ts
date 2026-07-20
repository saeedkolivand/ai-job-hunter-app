/**
 * Shared form-field signal primitives — label text, visibility, and the
 * ambiguous/sensitive denylist. Factored out of `autofill.ts` (PR 5 of the
 * extension roadmap) so BOTH `fill.ts` (assisted autofill, via
 * `autofill.ts`) and `capture.ts` (answers capture, via
 * `answers-capture.ts`) share ONE definition of "what counts as a labelled /
 * visible / ambiguous field" — never two copies that could drift.
 *
 * Pure DOM — no extension APIs, no network — so it is unit-testable against a
 * jsdom document. Behavior is UNCHANGED from what previously lived inline in
 * `autofill.ts`; this is a pure extraction (its existing tests pin the
 * behavior and pass unmodified).
 *
 * Build note: `fill.js` and `capture.js` are each injected via
 * `chrome.scripting.executeScript({ files: [...] })` as CLASSIC scripts (no
 * ES module support) — they must bundle with ZERO `import` statements. Since
 * BOTH now genuinely share this module at runtime, `vite.config.ts` builds
 * each in its OWN isolated Rollup pass (the `injectedEntries` plugin) so this
 * file is inlined into EACH bundle rather than hoisted into a shared chunk
 * that either classic script would then have to `import`.
 */

/**
 * Substrings that make a field ambiguous or sensitive — a match on the label /
 * name / id / placeholder skips the field entirely (under-fill over mis-fill
 * for autofill; skip-don't-capture for answers capture). Includes the grilled
 * set (referrer/emergency/confirm/manager/parent) plus the fields most likely
 * to receive the WRONG identity on a job-application form
 * (company/employer/recruiter), login/search noise, and — defense-in-depth —
 * sensitive PII categories that should never be touched even by accident
 * (SSN/tax id, passport, date of birth, national id, driver's license,
 * bank/IBAN, visa status).
 */
export const AMBIGUOUS = [
  'referr',
  'referral',
  'reference',
  'emergency',
  'confirm',
  'manager',
  'supervisor',
  'parent',
  'guardian',
  'company',
  'employer',
  'organization',
  'organisation',
  'recruiter',
  'search',
  'username',
  'user name',
  'password',
  'captcha',
  'coupon',
  'promo',
  'maiden',
  'ssn',
  'social security',
  'tax',
  'passport',
  'dob',
  'birth',
  'date of birth',
  'national id',
  'national insurance',
  'license',
  'licence',
  'iban',
  'bank account',
  'routing number',
  'sort code',
  'visa status',
  'green card',
  'immigration status',
  // ── Localized third-party / reference / emergency-contact fields ──────────
  // The EU-language equivalents of the referrer/reference/emergency/supervisor/
  // contact-person categories already protected in English above. Without
  // these, a "Name des Ansprechpartners" / "Name der Referenzperson" field
  // matches the generic `\bname\b` catch-all in `matchNamedKey` and mis-fills
  // with the applicant's OWN name. All are long, distinctive substrings with no
  // English-word collision (the short/collision-prone ones live in
  // {@link AMBIGUOUS_WORDS} below).
  'ansprechpartner', // DE contact person
  'notfall', // DE emergency (covers notfallkontakt/notfallnummer)
  'referenz', // DE reference (covers referenzperson/referenznummer)
  'vorgesetzt', // DE supervisor (vorgesetzte/vorgesetzter)
  'personne a contacter', // FR person to contact
  'recruteur', // FR recruiter
  'contacto de emergencia', // ES emergency contact
  'persona de contacto', // ES contact person
  'referencia', // ES reference
  'emergencia', // ES emergency
  'reclutador', // ES recruiter
  'contatto di emergenza', // IT emergency contact
  'referente', // IT reference/contact person
  'emergenza', // IT emergency
  'contactpersoon', // NL contact person
  'noodcontact', // NL emergency contact
  'osoba kontaktowa', // PL contact person
  'kontakt alarmowy', // PL emergency contact
  'referencje', // PL references
  // ── Localized sensitive PII (date of birth / national-id / tax / social) ──
  // Long, distinctive substrings (accent-free — the signal is diacritic-folded
  // by `textSignal`). A false match here only ever SKIPS a field (never
  // mis-fills / mis-captures), so this is the safe direction.
  'geburtsdatum', // DE date of birth
  'geburtstag', // DE birthday
  'steuernummer', // DE tax number
  'sozialversicherung', // DE social security
  'ausweis', // DE id card
  'date de naissance', // FR date of birth
  'numero de securite sociale', // FR social-security number
  'fecha de nacimiento', // ES date of birth
  'codice fiscale', // IT tax code
  'data di nascita', // IT date of birth
  'geboortedatum', // NL date of birth
  'data urodzenia', // PL date of birth
  'pesel', // PL national id
  'data de nascimento', // PT date of birth
  'personnummer', // SV/NO national id
  'cpr-nummer', // DA national id
  'fodselsnummer', // NO national id (ø folded to o by `stripDiacritics`)
  'henkilotunnus', // FI national id
];

/**
 * Ambiguous/sensitive terms that are too SHORT or too English-collision-prone
 * to match as plain substrings — matched on `\b` WORD boundaries instead (the
 * signal is accent-free and lowercased, so keywords are too). A bare substring
 * would wrongly SKIP a legitimate field: `dni` ⊂ Polish "poprzedni", `bsn` is a
 * 3-letter fragment, `urgence` ⊂ "insurgence". `\burgence\b` still catches the
 * FR "contact d'urgence" form (the apostrophe is a word boundary).
 */
const AMBIGUOUS_WORDS = /\b(?:dni|bsn|urgence)\b/;

/**
 * True when a field's {@link textSignal} is ambiguous or sensitive and must be
 * SKIPPED by both autofill ({@link textSignal} → `isCandidateField`) and
 * answers-capture (`isCapturable`). Combines the plain-substring {@link AMBIGUOUS}
 * denylist with the word-anchored {@link AMBIGUOUS_WORDS} one, so the two
 * consumers can never disagree on what counts as ambiguous.
 */
export function isAmbiguousSignal(signal: string): boolean {
  return AMBIGUOUS.some((w) => signal.includes(w)) || AMBIGUOUS_WORDS.test(signal);
}

/**
 * True when `el` or ANY ancestor is hidden — via the `hidden` attribute or
 * COMPUTED style (not just inline `style`): `display:none`/`visibility:hidden`,
 * `opacity:0`, off-screen absolute/fixed positioning (`left`/`top` shoved past
 * -9999px — the classic honeypot trap), or a box whose computed `width` AND
 * `height` are BOTH exactly `0px`. Computed style (not just inline `style`) is
 * what catches an external-stylesheet / `<style>` CSS-class honeypot — this is
 * how anti-bot honeypot fields are commonly planted on real ATS forms
 * (Greenhouse/Lever/Workday). An inline-only or display/visibility-only check
 * would fill/capture them, and a filled invisible field is worse than an
 * ordinary mis-fill (the user can't see it to undo, and it can flag them as a
 * bot).
 *
 * NOT caught, deliberately: clip-based hiding (`clip:rect(0,0,0,0)`/
 * `clip-path`) or a single-dimension-zero box (e.g. the `width:1px;height:1px`
 * shape common to `.sr-only`-style utility classes) — that is also exactly how a
 * LEGITIMATE screen-reader-only field is hidden visually while staying
 * functionally real, so treating it as hidden (and skipping it) would be
 * wrong. Only an unambiguous honeypot shape — display/visibility/opacity-off,
 * off-screen, or BOTH dimensions zero — is treated as hidden.
 *
 * Deliberately `getComputedStyle`-ONLY — never `getBoundingClientRect`/
 * `offsetWidth`/layout reads. jsdom (the test environment) always reports those
 * as zero, which would make every field — including normal visible ones — read
 * as hidden. Computed style has no such gap: a real field's computed `width` is
 * `auto`/a real length (never the literal string `'0px'`), its `position` is
 * `static`, and its `opacity` is `1`, so this stays jsdom-safe.
 */
export function isHidden(el: HTMLElement): boolean {
  const view = el.ownerDocument.defaultView;
  let node: HTMLElement | null = el;
  while (node) {
    if (node.hidden) return true;
    const cs = view?.getComputedStyle(node);
    if (cs) {
      if (cs.display === 'none' || cs.visibility === 'hidden') return true;
      if (Number.parseFloat(cs.opacity) === 0) return true;
      if (
        (cs.position === 'absolute' || cs.position === 'fixed') &&
        (Number.parseFloat(cs.left) <= -9999 || Number.parseFloat(cs.top) <= -9999)
      )
        return true;
      if (cs.width === '0px' && cs.height === '0px') return true;
    }
    node = node.parentElement;
  }
  return false;
}

/** CSS.escape when available (jsdom + browsers), else a conservative fallback. */
export function escapeId(id: string): string {
  if (typeof CSS !== 'undefined' && typeof CSS.escape === 'function') return CSS.escape(id);
  return id.replace(/["\\]/g, '\\$&');
}

/** The associated label text for a form element: `<label for>` + any wrapping
 *  `<label>`. Takes `HTMLElement` (not just `HTMLInputElement`) so it works
 *  identically for `<textarea>`/`<select>` — every member it touches
 *  (`id`/`closest`) is generic to `Element`, not input-specific. */
export function labelText(el: HTMLElement): string {
  const doc = el.ownerDocument;
  let text = '';
  if (el.id) {
    const forLabel = doc.querySelector(`label[for="${escapeId(el.id)}"]`);
    if (forLabel?.textContent) text += ` ${forLabel.textContent}`;
  }
  const wrapping = el.closest('label');
  if (wrapping?.textContent) text += ` ${wrapping.textContent}`;
  return text;
}

/** Fold an accented EU label to ASCII so it matches the accent-free keyword
 *  table in {@link matchNamedKey} and the {@link AMBIGUOUS} denylist. Two steps,
 *  because NFD alone is not enough: (1) NFD-decompose then strip combining marks
 *  (é → e, ä → a, ż → z, å → a); (2) an explicit fold for the atomic letters
 *  that have NO NFD decomposition (ø, æ, ł, đ, ß) — without step 2, "Fødselsnummer"
 *  or "Szkoła" would keep their ø/ł and never match "fodselsnummer"/"szkola".
 *  Kept LOCAL to this file (no import — this module is inlined verbatim into the
 *  classic-injected `fill.js`/`capture.js` bundles, which forbid `import`);
 *  `autofill.ts` keeps its own `normalizeLabel` for the extra-link matcher. */
function stripDiacritics(s: string): string {
  return s
    .normalize('NFD')
    .replace(/\p{Diacritic}/gu, '')
    .replace(/[øØ]/g, 'o')
    .replace(/[æÆ]/g, 'ae')
    .replace(/[łŁ]/g, 'l')
    .replace(/[đĐ]/g, 'd')
    .replace(/[ßẞ]/g, 'ss');
}

/** The accent-free, lowercased free-text signal (name/id/placeholder/aria-label/
 *  label) used both for autofill's Tier-2 field matching and the answers-capture
 *  denylist check. Diacritics are stripped here (not per keyword) so BOTH the
 *  named-key table and the AMBIGUOUS denylist can be written accent-free and a
 *  German/French/Polish/… label still matches. Takes `HTMLElement` for the same
 *  reason as `labelText`. */
export function textSignal(el: HTMLElement): string {
  return stripDiacritics(
    [
      el.getAttribute('name') ?? '',
      el.id,
      el.getAttribute('placeholder') ?? '',
      el.getAttribute('aria-label') ?? '',
      labelText(el),
    ].join(' ')
  ).toLowerCase();
}

/** The last (field) token of an `autocomplete` attribute value, e.g.
 *  "shipping email" → "email". `''` for a missing/`off`/`on` attribute. Takes
 *  `HTMLElement` (not just `HTMLInputElement`) so `answers-capture.ts` can call
 *  it on a `<textarea>`/`<select>` too — an `autocomplete` attribute there
 *  simply normalizes to `''`/`off`, same as no match. Shared so autofill's
 *  Tier-1 token reading and answers-capture's identity check never drift on
 *  what the raw token is. */
export function autocompleteToken(el: HTMLElement): string {
  const raw = (el.getAttribute('autocomplete') ?? '').trim().toLowerCase();
  if (!raw || raw === 'off' || raw === 'on') return '';
  return raw.split(/\s+/).at(-1) ?? '';
}

/**
 * Map a standard `autocomplete` {@link autocompleteToken} to autofill's Tier-1
 * logical key, or `null` for a token with no fill/identity meaning here.
 * Mirrors `matchFieldKey`'s Tier-1 switch (`autofill.ts`) — factored out here
 * so `isCapturable` (`answers-capture.ts`) can exclude a field whose
 * `autocomplete` attribute marks it as identity (e.g. `autocomplete="name"`)
 * WITHOUT duplicating the token→key literals in a second copy.
 */
export function matchAutocompleteKey(token: string): string | null {
  switch (token) {
    case 'email':
      return 'email';
    case 'tel':
    case 'tel-national':
    case 'tel-local':
      return 'phone';
    case 'given-name':
      return 'firstName';
    case 'family-name':
      return 'lastName';
    case 'name':
      return 'fullName';
    case 'url':
      return 'website';
    // Only the city-level address token maps to the single free-text location;
    // street/postal/state/country sub-parts can't be filled from one string.
    case 'address-level2':
      return 'location';
    default:
      return null;
  }
}

/**
 * Resolve a lowercased field {@link textSignal} to a known identity key, or
 * `null` when it doesn't unambiguously match one. This is autofill's "Tier 2"
 * signal matching (`matchFieldKey` in `autofill.ts`), factored out so it's
 * shared with answers-capture: `isCapturable` (`answers-capture.ts`) calls it
 * to EXCLUDE any field whose signal identifies it as one of these keys — a
 * filled "Full Name" or "LinkedIn URL" text field must never be captured into
 * `Application.answers` as if it were a genuine application question. Pure
 * string matching — no element/autocomplete-attribute lookup (that stays
 * `autofill.ts`-only "Tier 1", since capture also runs against `<select>`/
 * `<textarea>` which don't carry the same autocomplete semantics).
 */
/**
 * Ordered, first-match-wins keyword table for {@link matchNamedKey}. Each
 * `pattern` runs against the accent-free, lowercased {@link textSignal}, so
 * every keyword is written WITHOUT diacritics (universite, not université) and
 * a `\b` anchor is used for short / collision-prone terms.
 *
 * Widened beyond English to the major EU languages
 * (DE/FR/ES/IT/NL/PL/PT/SV/DA, plus NO/FI where the words coincide). Two
 * ordering choices matter:
 *  - the COMBINED full-name phrases ("nombre completo", "imie i nazwisko", …)
 *    run BEFORE first/last, so such a field resolves to `fullName` rather than
 *    grabbing just its first (or last) token — those single tokens are
 *    substrings of the combined phrase;
 *  - a name term that is also a substring of a "username"/"company"/full-name
 *    phrase carries a negative lookahead (e.g. `nombre(?!… de …)`) so a
 *    "Nombre de usuario" / "Nombre de la empresa" field never mis-fills as a
 *    first name.
 *
 * The generic bare-"Name" catch-all (with its education/company/user denylist)
 * is NOT in this table — it runs last, in {@link matchNamedKey}, only after
 * every specific pattern misses.
 */
const NAMED_KEY_PATTERNS: readonly { key: string; pattern: RegExp }[] = [
  { key: 'linkedin', pattern: /linkedin/ },
  { key: 'github', pattern: /github/ },
  { key: 'website', pattern: /portfolio|personal (web ?site|site)/ },
  // `email`/`e-mail` already cover most EU forms (e-mail-adresse, adresse
  // e-mail, indirizzo e-mail, …); only the non-"mail" spellings are added.
  { key: 'email', pattern: /email|e-mail|\bcorreo\b|\bcourriel\b|sahkoposti/ },
  // `telefon` (substring) covers telefon(nummer)/telefono/telefone across
  // DE/ES/IT/PT/SV/DA/NO/PL; `telefoon` (NL) and `telephone` (FR/EN) differ.
  // `handy`/`mobil` stay `\b`-anchored (bare `handy` ⊂ "handyman", bare `mobil`
  // ⊂ "automobil…"), so the concatenated DE compounds are listed explicitly.
  {
    key: 'phone',
    pattern:
      /phone|mobile|telephone|telefon|telefoon|handynummer|handytelefon|mobilnummer|mobiltelefon|\bhandy\b|\bmobil\b|puhelin/,
  },
  // Combined full-name phrases — MUST precede first/last (see table doc).
  {
    key: 'fullName',
    pattern:
      /\bfull name\b|vollstandiger name|nom complet|nombre completo|nome completo|imie i nazwisko|volledige naam|fullstandigt namn/,
  },
  // `nombre` (ES) and `nome` (IT/PT) mean "name" — excluded when they head a
  // username/company/full-name phrase so they only fire for a real first name.
  {
    key: 'firstName',
    pattern:
      /first name|given name|forename|vorname|prenom|voornaam|fornamn|fornavn|etunimi|\bimie\b|\bnombre\b(?!\s*(?:de\b|completo))|\bnome\b(?!\s*(?:completo|utente|de\b|da\b|del))/,
  },
  {
    key: 'lastName',
    pattern:
      /last name|surname|family name|nachname|familienname|nom de famille|\bapellidos?\b|cognome|achternaam|nazwisko|apelido|sobrenome|efternamn|efternavn|etternavn|sukunimi/,
  },
  // `city`/`town` stay plain substrings (unchanged English behavior); the added
  // EU city/place terms use `\b` where they are short/collision-prone.
  {
    key: 'location',
    pattern:
      /city|town|\blocation\b|\bort\b|stadt|wohnort|\bville\b|ciudad|citta|plaats|miasto|cidade|localidad/,
  },
];

export function matchNamedKey(signal: string): string | null {
  for (const { key, pattern } of NAMED_KEY_PATTERNS) {
    if (pattern.test(signal)) return key;
  }
  // Generic catch-all: a bare "Name" field → full name, UNLESS it's a
  // user/file/nick/display/business/org field OR a school/company/user-account
  // "name" field. The denylist is localized (accent-free — the signal is
  // diacritic-stripped) so "Name der Schule" / "Name des Unternehmens" /
  // "Nom de l'entreprise"-style fields never receive the person's name.
  if (
    /\bname\b/.test(signal) &&
    // `\bfirma\b` is anchored — a bare `firma` substring wrongly matches the
    // English words "affirmative"/"confirmation", which would stop a legitimate
    // "Name (Affirmative Action)" EEO field from filling.
    !/user|file|nick|screen|display|business|org|school|institution|university|college|degree|course|program|certificat|schule|hochschule|universitat|benutzer|\bfirma\b|unternehmen|ecole|universite|entreprise|societe|utilisateur|escuela|universidad|empresa|usuario|scuola|universita|azienda|utente|szkola|uczelnia|uzytkownik|gebruiker|bedrijf|foretag|anvandare|virksomhed|bruger|yritys|kayttaja/.test(
      signal
    )
  )
    return 'fullName';

  return null;
}
